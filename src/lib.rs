//! Basic SPIR-V reflection library to extract binding information
//!
//! ```no_run
//! # let spirv_blob: &[u8] = todo!();
//! let info = rspirv_reflect::Reflection::new_from_spirv(&spirv_blob).expect("Invalid SPIR-V");
//! dbg!(info.get_descriptor_sets().expect("Failed to extract descriptor bindings"));
//! ```

use rspirv::binary::Parser;
use rspirv::dr::{Instruction, Loader, Module, Operand};
use std::collections::BTreeMap;
use std::convert::TryInto;
use std::num::TryFromIntError;
use thiserror::Error;

pub use rspirv;
pub use rspirv::spirv;

pub struct Reflection(pub Module);

#[derive(Error, Debug)]
pub enum ReflectError {
    // NOTE: Instructions are stored as string because they cannot be cloned,
    // and storing a reference means the shader must live at least as long as
    // the error bubbling up, which is generally impossible.
    #[error("{0:?} missing binding decoration")]
    MissingBindingDecoration(Instruction),
    #[error("{0:?} missing set decoration")]
    MissingSetDecoration(Instruction),
    #[error("Expecting operand {1} in position {2} for instruction {0:?}")]
    OperandError(Instruction, &'static str, usize),
    #[error("Expecting operand {1} in position {2} but instruction {0:?} has only {3} operands")]
    OperandIndexError(Instruction, &'static str, usize, usize),
    #[error("OpVariable {0:?} lacks a return type")]
    VariableWithoutReturnType(Instruction),
    #[error("Unknown storage class {0:?}")]
    UnknownStorageClass(spirv::StorageClass),
    #[error("Unknown struct (missing Block or BufferBlock annotation): {0:?}")]
    UnknownStruct(Instruction),
    #[error("Unknown value {1} for `sampled` field: {0:?}")]
    ImageSampledFieldUnknown(Instruction, u32),
    #[error("Unhandled OpType instruction {0:?}")]
    UnhandledTypeInstruction(Instruction),
    #[error("{0:?} does not generate a result")]
    MissingResultId(Instruction),
    #[error("No instruction assigns to {0:?}")]
    UnassignedResultId(u32),
    #[error("rspirv reflect lacks module header")]
    MissingHeader,
    #[error("Accidentally binding global parameter buffer. Global variables are currently not supported in HLSL")]
    BindingGlobalParameterBuffer,
    #[error("Only one push constant block can be defined per shader entry")]
    TooManyPushConstants,
    #[error("SPIR-V parse error")]
    ParseError(#[from] rspirv::binary::ParseState),
    #[error("OpTypeInt cannot have width {0}")]
    UnexpectedIntWidth(u32),
    #[error(transparent)]
    TryFromIntError(#[from] TryFromIntError),
}

type Result<V, E = ReflectError> = ::std::result::Result<V, E>;

/// These are bit-exact with ash and the Vulkan specification,
/// they're mirrored here to prevent a dependency on ash
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct DescriptorType(pub u32);

// TODO: Possibly change to a C-like enum to get automatic Debug?
impl DescriptorType {
    pub const SAMPLER: Self = Self(0);
    pub const COMBINED_IMAGE_SAMPLER: Self = Self(1);
    pub const SAMPLED_IMAGE: Self = Self(2);
    pub const STORAGE_IMAGE: Self = Self(3);
    pub const UNIFORM_TEXEL_BUFFER: Self = Self(4);
    pub const STORAGE_TEXEL_BUFFER: Self = Self(5);
    pub const UNIFORM_BUFFER: Self = Self(6);
    pub const STORAGE_BUFFER: Self = Self(7);
    pub const UNIFORM_BUFFER_DYNAMIC: Self = Self(8);
    pub const STORAGE_BUFFER_DYNAMIC: Self = Self(9);
    pub const INPUT_ATTACHMENT: Self = Self(10);

    pub const INLINE_UNIFORM_BLOCK_EXT: Self = Self(1_000_138_000);
    pub const ACCELERATION_STRUCTURE_KHR: Self = Self(1_000_150_000);
    pub const ACCELERATION_STRUCTURE_NV: Self = Self(1_000_165_000);
}

impl std::fmt::Debug for DescriptorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match *self {
            Self::SAMPLER => "SAMPLER",
            Self::COMBINED_IMAGE_SAMPLER => "COMBINED_IMAGE_SAMPLER",
            Self::SAMPLED_IMAGE => "SAMPLED_IMAGE",
            Self::STORAGE_IMAGE => "STORAGE_IMAGE",
            Self::UNIFORM_TEXEL_BUFFER => "UNIFORM_TEXEL_BUFFER",
            Self::STORAGE_TEXEL_BUFFER => "STORAGE_TEXEL_BUFFER",
            Self::UNIFORM_BUFFER => "UNIFORM_BUFFER",
            Self::STORAGE_BUFFER => "STORAGE_BUFFER",
            Self::UNIFORM_BUFFER_DYNAMIC => "UNIFORM_BUFFER_DYNAMIC",
            Self::STORAGE_BUFFER_DYNAMIC => "STORAGE_BUFFER_DYNAMIC",
            Self::INPUT_ATTACHMENT => "INPUT_ATTACHMENT",
            Self::INLINE_UNIFORM_BLOCK_EXT => "INLINE_UNIFORM_BLOCK_EXT",
            Self::ACCELERATION_STRUCTURE_KHR => "ACCELERATION_STRUCTURE_KHR",
            Self::ACCELERATION_STRUCTURE_NV => "ACCELERATION_STRUCTURE_NV",
            _ => "(UNDEFINED)",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingCount {
    /// A single resource binding.
    ///
    /// # Example
    /// ```hlsl
    /// StructuredBuffer<uint>
    /// ```
    One,
    /// Predetermined number of resource bindings.
    ///
    /// # Example
    /// ```hlsl
    /// StructuredBuffer<uint> myBinding[4]
    /// ```
    StaticSized(usize),
    /// Variable number of resource bindings (usually dubbed "bindless").
    ///
    /// Count is determined in `vkDescriptorSetLayoutBinding`. No other bindings should follow in this set.
    ///
    /// # Example
    /// ```hlsl
    /// StructuredBuffer<uint> myBinding[]
    /// ```
    Unbounded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorInfo {
    pub ty: DescriptorType,
    pub binding_count: BindingCount,
    pub name: String,
}

pub struct PushConstantInfo {
    pub offset: u32,
    pub size: u32,
}

macro_rules! get_ref_operand_at {
    // TODO: Can't we have a match arm that deals with `ops` containing `&instruction.operands`?
    ($instr:expr, $op:path, $idx:expr) => {
        if $idx >= $instr.operands.len() {
            Err(ReflectError::OperandIndexError(
                $instr.clone(),
                stringify!($op),
                $idx,
                $instr.operands.len(),
            ))
        } else if let $op(val) = &$instr.operands[$idx] {
            Ok(val)
        } else {
            Err(ReflectError::OperandError(
                $instr.clone(),
                stringify!($op),
                $idx,
            ))
        }
    };
}

macro_rules! get_operand_at {
    ($ops:expr, $op:path, $idx:expr) => {
        get_ref_operand_at!($ops, $op, $idx).map(|v| *v)
    };
}

impl Reflection {
    pub fn new(module: Module) -> Self {
        Self(module)
    }

    pub fn new_from_spirv(code: &[u8]) -> Result<Self> {
        Ok(Self::new({
            let mut loader = Loader::new();
            let p = Parser::new(code, &mut loader);
            p.parse()?;
            loader.module()
        }))
    }

    /// Returns all instructions where the first operand (`Instruction::operands[0]`) equals `IdRef(id)`
    pub fn find_annotations_for_id(
        annotations: &[Instruction],
        id: u32,
    ) -> Result<Vec<&Instruction>> {
        annotations
            .iter()
            .filter_map(|a| {
                let op = get_operand_at!(a, Operand::IdRef, 0);
                match op {
                    Ok(idref) if idref == id => Some(Ok(a)),
                    Err(e) => Some(Err(e)),
                    _ => None,
                }
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Returns the first `Instruction` assigning to `id` (ie. `result_id == Some(id)`)
    pub fn find_assignment_for(instructions: &[Instruction], id: u32) -> Result<&Instruction> {
        // TODO: Find unique?
        instructions
            .iter()
            .find(|instr| instr.result_id == Some(id))
            .ok_or(ReflectError::UnassignedResultId(id))
    }

    pub fn get_compute_group_size(&self) -> Option<(u32, u32, u32)> {
        for inst in self.0.global_inst_iter() {
            if inst.class.opcode == spirv::Op::ExecutionMode {
                use rspirv::dr::Operand::{ExecutionMode, LiteralInt32};
                if let [ExecutionMode(
                    spirv::ExecutionMode::LocalSize | spirv::ExecutionMode::LocalSizeHint,
                ), LiteralInt32(x), LiteralInt32(y), LiteralInt32(z)] = inst.operands[1..]
                {
                    return Some((x, y, z));
                } else {
                    // Invalid encoding? Ignoring.
                }
            }
        }
        None
    }

    /// Returns the descriptor type for a given variable `type_id`
    fn get_descriptor_type_for_var(
        &self,
        type_id: u32,
        storage_class: spirv::StorageClass,
    ) -> Result<DescriptorInfo> {
        let type_instruction = Self::find_assignment_for(&self.0.types_global_values, type_id)?;
        self.get_descriptor_type(type_instruction, storage_class)
    }

    /// Returns the descriptor type for a given `OpType*` `Instruction`
    fn get_descriptor_type(
        &self,
        type_instruction: &Instruction,
        storage_class: spirv::StorageClass,
    ) -> Result<DescriptorInfo> {
        let annotations = type_instruction.result_id.map_or(Ok(vec![]), |result_id| {
            Reflection::find_annotations_for_id(&self.0.annotations, result_id)
        })?;

        // Weave with recursive types
        match type_instruction.class.opcode {
            spirv::Op::TypeArray => {
                let element_type_id = get_operand_at!(type_instruction, Operand::IdRef, 0)?;
                let num_elements_id = get_operand_at!(type_instruction, Operand::IdRef, 1)?;
                let num_elements =
                    Self::find_assignment_for(&self.0.types_global_values, num_elements_id)?;
                assert_eq!(num_elements.class.opcode, spirv::Op::Constant);
                let num_elements_ty = Self::find_assignment_for(
                    &self.0.types_global_values,
                    num_elements.result_type.unwrap(),
                )?;
                // Array size can be any width, any signedness
                assert_eq!(num_elements_ty.class.opcode, spirv::Op::TypeInt);
                let num_elements = match get_operand_at!(num_elements_ty, Operand::LiteralInt32, 0)?
                {
                    32 => get_operand_at!(num_elements, Operand::LiteralInt32, 0)?.try_into()?,
                    64 => get_operand_at!(num_elements, Operand::LiteralInt64, 0)?.try_into()?,
                    x => return Err(ReflectError::UnexpectedIntWidth(x)),
                };
                assert!(num_elements >= 1);
                return Ok(DescriptorInfo {
                    binding_count: BindingCount::StaticSized(num_elements),
                    ..self.get_descriptor_type_for_var(element_type_id, storage_class)?
                });
            }
            spirv::Op::TypeRuntimeArray => {
                let element_type_id = get_operand_at!(type_instruction, Operand::IdRef, 0)?;
                return Ok(DescriptorInfo {
                    binding_count: BindingCount::Unbounded,
                    ..self.get_descriptor_type_for_var(element_type_id, storage_class)?
                });
            }
            spirv::Op::TypePointer => {
                let ptr_storage_class =
                    get_operand_at!(type_instruction, Operand::StorageClass, 0)?;
                let element_type_id = get_operand_at!(type_instruction, Operand::IdRef, 1)?;
                assert_eq!(storage_class, ptr_storage_class);
                return self.get_descriptor_type_for_var(element_type_id, storage_class);
            }
            spirv::Op::TypeSampledImage => {
                let element_type_id = get_operand_at!(type_instruction, Operand::IdRef, 0)?;

                let image_instruction =
                    Self::find_assignment_for(&self.0.types_global_values, element_type_id)?;

                let descriptor = self.get_descriptor_type(image_instruction, storage_class)?;

                let dim = get_operand_at!(image_instruction, Operand::Dim, 1)?;
                assert_ne!(dim, spirv::Dim::DimSubpassData);

                return Ok(if dim == spirv::Dim::DimBuffer {
                    if descriptor.ty != DescriptorType::UNIFORM_TEXEL_BUFFER
                        && descriptor.ty != DescriptorType::STORAGE_TEXEL_BUFFER
                    {
                        todo!("Unexpected sampled image type {:?}", descriptor.ty)
                    }
                    descriptor
                } else {
                    DescriptorInfo {
                        ty: DescriptorType::COMBINED_IMAGE_SAMPLER,
                        ..descriptor
                    }
                });
            }
            _ => {}
        }

        let descriptor_type = match type_instruction.class.opcode {
            spirv::Op::TypeSampler => DescriptorType::SAMPLER,
            spirv::Op::TypeImage => {
                let dim = get_operand_at!(type_instruction, Operand::Dim, 1)?;

                const IMAGE_SAMPLED: u32 = 1;
                const IMAGE_STORAGE: u32 = 2;

                // TODO: Should this be modeled as an enum in rspirv??
                let sampled = get_operand_at!(type_instruction, Operand::LiteralInt32, 5)?;

                if dim == spirv::Dim::DimBuffer {
                    if sampled == IMAGE_SAMPLED {
                        DescriptorType::UNIFORM_TEXEL_BUFFER
                    } else if sampled == IMAGE_STORAGE {
                        DescriptorType::STORAGE_TEXEL_BUFFER
                    } else {
                        return Err(ReflectError::ImageSampledFieldUnknown(
                            type_instruction.clone(),
                            sampled,
                        ));
                    }
                } else if dim == spirv::Dim::DimSubpassData {
                    DescriptorType::INPUT_ATTACHMENT
                } else if sampled == IMAGE_SAMPLED {
                    DescriptorType::SAMPLED_IMAGE
                } else if sampled == IMAGE_STORAGE {
                    DescriptorType::STORAGE_IMAGE
                } else {
                    return Err(ReflectError::ImageSampledFieldUnknown(
                        type_instruction.clone(),
                        sampled,
                    ));
                }
            }
            spirv::Op::TypeStruct => {
                let mut is_uniform_buffer = false;
                let mut is_storage_buffer = false;

                for annotation in annotations {
                    for operand in &annotation.operands {
                        if let Operand::Decoration(decoration) = operand {
                            match decoration {
                                spirv::Decoration::Block => is_uniform_buffer = true,
                                spirv::Decoration::BufferBlock => is_storage_buffer = true,
                                _ => { /* println!("Unhandled decoration {:?}", decoration) */ }
                            }
                        }
                    }
                }

                let version = self
                    .0
                    .header
                    .as_ref()
                    .ok_or(ReflectError::MissingHeader)?
                    .version();

                if version <= (1, 3) && is_storage_buffer {
                    // BufferBlock is still support in 1.3 exactly.
                    DescriptorType::STORAGE_BUFFER
                } else if version >= (1, 3) {
                    // From 1.3, StorageClass is supported.
                    assert!(
                        !is_storage_buffer,
                        "BufferBlock decoration is obsolete in SPIRV > 1.3"
                    );
                    assert!(
                        is_uniform_buffer,
                        "Struct requires Block annotation in SPIRV > 1.3"
                    );
                    match storage_class {
                        spirv::StorageClass::Uniform | spirv::StorageClass::UniformConstant => {
                            DescriptorType::UNIFORM_BUFFER
                        }
                        spirv::StorageClass::StorageBuffer => DescriptorType::STORAGE_BUFFER,
                        _ => return Err(ReflectError::UnknownStorageClass(storage_class)),
                    }
                } else if is_uniform_buffer {
                    DescriptorType::UNIFORM_BUFFER
                } else {
                    return Err(ReflectError::UnknownStruct(type_instruction.clone()));
                }
            }
            // TODO: spirv_reflect translates nothing to {UNIFORM,STORAGE}_BUFFER_DYNAMIC
            spirv::Op::TypeAccelerationStructureKHR => DescriptorType::ACCELERATION_STRUCTURE_KHR,
            _ => {
                return Err(ReflectError::UnhandledTypeInstruction(
                    type_instruction.clone(),
                ))
            }
        };

        Ok(DescriptorInfo {
            ty: descriptor_type,
            binding_count: BindingCount::One,
            name: "".to_string(),
        })
    }

    /// Returns a nested mapping, where the first level maps descriptor set indices (register spaces)
    /// and the second level maps descriptor binding indices (registers) to descriptor information.
    pub fn get_descriptor_sets(&self) -> Result<BTreeMap<u32, BTreeMap<u32, DescriptorInfo>>> {
        let mut unique_sets = BTreeMap::new();
        let reflect = &self.0;

        let uniform_variables = reflect
            .types_global_values
            .iter()
            .filter(|i| i.class.opcode == spirv::Op::Variable)
            .filter_map(|i| {
                let cls = get_operand_at!(i, Operand::StorageClass, 0);
                match cls {
                    Ok(cls)
                        if cls == spirv::StorageClass::Uniform
                            || cls == spirv::StorageClass::UniformConstant
                            || cls == spirv::StorageClass::StorageBuffer =>
                    {
                        Some(Ok(i))
                    }
                    Err(e) => Some(Err(e)),
                    _ => None,
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        let names = reflect
            .debug_names
            .iter()
            .filter(|i| i.class.opcode == spirv::Op::Name)
            .map(|i| -> Result<(u32, String)> {
                let element_type_id = get_operand_at!(i, Operand::IdRef, 0)?;
                let name = get_ref_operand_at!(i, Operand::LiteralString, 1)?;
                Ok((element_type_id, name.clone()))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;

        for var in uniform_variables {
            if let Some(var_id) = var.result_id {
                let annotations =
                    Reflection::find_annotations_for_id(&reflect.annotations, var_id)?;

                // TODO: Can also define these as mut
                let (set, binding) = annotations.iter().filter(|a| a.operands.len() >= 3).fold(
                    (None, None),
                    |state, a| {
                        if let Operand::Decoration(d) = a.operands[1] {
                            if let Operand::LiteralInt32(i) = a.operands[2] {
                                if d == spirv::Decoration::DescriptorSet {
                                    assert!(state.0.is_none(), "Set already has a value!");
                                    return (Some(i), state.1);
                                } else if d == spirv::Decoration::Binding {
                                    assert!(state.1.is_none(), "Binding already has a value!");
                                    return (state.0, Some(i));
                                }
                            }
                        }
                        state
                    },
                );

                let set = set.ok_or_else(|| ReflectError::MissingSetDecoration(var.clone()))?;
                let binding =
                    binding.ok_or_else(|| ReflectError::MissingBindingDecoration(var.clone()))?;

                let current_set = /* &mut */ unique_sets
                    .entry(set)
                    .or_insert_with(BTreeMap::<u32, DescriptorInfo>::new);

                let storage_class = get_operand_at!(var, Operand::StorageClass, 0)?;

                let type_id = var
                    .result_type
                    .ok_or_else(|| ReflectError::VariableWithoutReturnType(var.clone()))?;
                let mut descriptor_info =
                    self.get_descriptor_type_for_var(type_id, storage_class)?;

                if let Some(name) = names.get(&var_id) {
                    // TODO: Might do this way earlier
                    if name.eq(&"$Globals") {
                        return Err(ReflectError::BindingGlobalParameterBuffer);
                    }

                    descriptor_info.name = (*name).clone();
                }

                let inserted = current_set.insert(binding, descriptor_info);
                assert!(
                    inserted.is_none(),
                    "Can't bind to the same slot twice within the same shader"
                );
            }
        }
        Ok(unique_sets)
    }

    fn byte_offset_to_last_var(
        reflect: &Module,
        struct_instruction: &Instruction,
    ) -> Result<u32, ReflectError> {
        debug_assert!(struct_instruction.class.opcode == spirv::Op::TypeStruct);

        // if there are less then two members there is no offset to use, early out
        if struct_instruction.operands.len() < 2 {
            return Ok(0);
        }

        let result_id = struct_instruction
            .result_id
            .ok_or_else(|| ReflectError::MissingResultId(struct_instruction.clone()))?;

        // return the highest offset value
        Ok(
            Self::find_annotations_for_id(&reflect.annotations, result_id)?
                .iter()
                .filter(|i| i.class.opcode == spirv::Op::MemberDecorate)
                .filter_map(|&i| match get_operand_at!(i, Operand::Decoration, 2) {
                    Ok(decoration) if decoration == spirv::Decoration::Offset => {
                        Some(get_operand_at!(i, Operand::LiteralInt32, 3))
                    }
                    Err(err) => Some(Err(err)),
                    _ => None,
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .max()
                .unwrap_or(0),
        )
    }

    fn calculate_variable_size_bytes(
        reflect: &Module,
        type_instruction: &Instruction,
    ) -> Result<u32, ReflectError> {
        match type_instruction.class.opcode {
            spirv::Op::TypeInt | spirv::Op::TypeFloat => {
                debug_assert!(!type_instruction.operands.is_empty());
                Ok(get_operand_at!(type_instruction, Operand::LiteralInt32, 0)? / 8)
            }
            spirv::Op::TypeVector | spirv::Op::TypeMatrix => {
                debug_assert!(type_instruction.operands.len() == 2);
                let type_id = get_operand_at!(type_instruction, Operand::IdRef, 0)?;
                let var_type_instruction =
                    Self::find_assignment_for(&reflect.types_global_values, type_id)?;
                let type_size_bytes =
                    Self::calculate_variable_size_bytes(reflect, var_type_instruction)?;

                let type_constant_count =
                    get_operand_at!(type_instruction, Operand::LiteralInt32, 1)?;
                Ok(type_size_bytes * type_constant_count)
            }
            spirv::Op::TypeArray => {
                debug_assert!(type_instruction.operands.len() == 2);
                let type_id = get_operand_at!(type_instruction, Operand::IdRef, 0)?;
                let var_type_instruction =
                    Self::find_assignment_for(&reflect.types_global_values, type_id)?;
                let type_size_bytes =
                    Self::calculate_variable_size_bytes(reflect, var_type_instruction)?;

                let var_constant_id = get_operand_at!(type_instruction, Operand::IdRef, 1)?;
                let constant_instruction =
                    Self::find_assignment_for(&reflect.types_global_values, var_constant_id)?;
                let type_constant_count =
                    get_operand_at!(constant_instruction, Operand::LiteralInt32, 0)?;

                Ok(type_size_bytes * type_constant_count)
            }
            spirv::Op::TypeStruct => {
                if !type_instruction.operands.is_empty() {
                    let byte_offset = Self::byte_offset_to_last_var(reflect, type_instruction)?;
                    let last_var_idx = type_instruction.operands.len() - 1;
                    let id_ref = get_operand_at!(type_instruction, Operand::IdRef, last_var_idx)?;
                    let type_instruction =
                        Self::find_assignment_for(&reflect.types_global_values, id_ref)?;
                    Ok(byte_offset
                        + Self::calculate_variable_size_bytes(reflect, type_instruction)?)
                } else {
                    Ok(0)
                }
            }
            _ => Ok(0),
        }
    }

    pub fn get_push_constant_range(&self) -> Result<Option<PushConstantInfo>, ReflectError> {
        let reflect = &self.0;

        let push_constants = reflect
            .types_global_values
            .iter()
            .filter(|i| i.class.opcode == spirv::Op::Variable)
            .filter_map(|i| {
                let cls = get_operand_at!(*i, Operand::StorageClass, 0);
                match cls {
                    Ok(cls) if cls == spirv::StorageClass::PushConstant => Some(Ok(i)),
                    Err(err) => Some(Err(err)),
                    _ => None,
                }
            })
            .collect::<Result<Vec<_>>>()?;

        if push_constants.len() > 1 {
            return Err(ReflectError::TooManyPushConstants);
        }

        let push_constant = match push_constants.into_iter().next() {
            Some(push_constant) => push_constant,
            None => return Ok(None),
        };

        let instruction = Reflection::find_assignment_for(
            &reflect.types_global_values,
            push_constant.result_type.unwrap(),
        )?;

        // resolve type if the type instruction is a pointer
        let instruction = if instruction.class.opcode == spirv::Op::TypePointer {
            let ptr_storage_class = get_operand_at!(instruction, Operand::StorageClass, 0)?;
            assert_eq!(spirv::StorageClass::PushConstant, ptr_storage_class);
            let element_type_id = get_operand_at!(instruction, Operand::IdRef, 1)?;
            Reflection::find_assignment_for(&reflect.types_global_values, element_type_id)?
        } else {
            instruction
        };

        let size_bytes = Self::calculate_variable_size_bytes(reflect, instruction)?;

        Ok(Some(PushConstantInfo {
            size: size_bytes,
            offset: 0,
        }))
    }

    pub fn disassemble(&self) -> String {
        use rspirv::binary::Disassemble;
        self.0.disassemble()
    }
}
