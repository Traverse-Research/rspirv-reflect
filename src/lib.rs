//! Basic SPIR-V reflection library to extract binding information
//!
//! ```rustc
//! let info = Reflection::new_from_spirv(&spirv_blob)?;
//! dbg!(info.get_descriptor_sets()?);
//! ```
use rspirv::binary::Parser;
use rspirv::dr::{Instruction, Loader, Module, Operand};
use std::collections::HashMap;
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
    #[error("No instruction assigns to {0:?}")]
    UnassignedResultId(u32),
    #[error("rspirv reflect lacks module header")]
    MissingHeader,
    #[error("Accidentally binding global parameter buffer. Global variables are currently not supported in HLSL")]
    BindingGlobalParameterBuffer,
    #[error("SPIR-V parse error")]
    ParseError(#[from] rspirv::binary::ParseState),
}

type Result<V, E = ReflectError> = ::std::result::Result<V, E>;

/// These are bit-exact with ash and the Vulkan specification,
/// they're mirrored here to prevent a dependency on ash
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct DescriptorType(pub u32);

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
    pub const ACCELERATION_STRUCTURE_KHR: Self = Self(1_000_165_000);
}

#[derive(Debug, Clone, PartialEq)]
pub struct DescriptorInfo {
    pub ty: DescriptorType,
    pub is_bindless: bool,
    pub name: String,
}

macro_rules! get_ref_operand_at {
    // TODO: Can't we have a match arm that deals with `ops` containing `&instruction.operands`?
    ($instr:expr, $op:path, $idx:expr) => {
        if let $op(val) = &$instr.operands[$idx] {
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
        get_ref_operand_at!($ops, $op, $idx)
            // Nightly: .as_deref()
            .map(|v| *v)
    };
}

impl Reflection {
    pub fn new(module: Module) -> Self {
        Self(module)
    }

    pub fn new_from_spirv(code: &[u8]) -> Result<Self, ReflectError> {
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
            .ok_or_else(|| ReflectError::UnassignedResultId(id))
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
            spirv::Op::TypeRuntimeArray => {
                let element_type_id = get_operand_at!(type_instruction, Operand::IdRef, 0)?;
                return Ok(DescriptorInfo {
                    is_bindless: true,
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
            spirv::Op::TypeSampledImage => {
                todo!("{:?} Not implemented; untested", type_instruction.class);
                // Note that `dim`, `sampled` and `storage` are parsed from TypeImage
                // if dim == SpvDimBuffer {
                //     if sampled {
                //         DescriptorType::UNIFORM_TEXEL_BUFFER
                //     } else if storage {
                //         DescriptorType::STORAGE_TEXEL_BUFFER
                //     }
                // } else {
                //     DescriptorType::COMBINED_IMAGE_SAMPLER
                // }
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

                if self
                    .0
                    .header
                    .as_ref()
                    .ok_or_else(|| ReflectError::MissingHeader)?
                    .version()
                    > (1, 3)
                {
                    assert_eq!(
                        is_storage_buffer, false,
                        "BufferBlock decoration is obsolete in SPIRV > 1.3"
                    );
                    assert_eq!(
                        is_uniform_buffer, true,
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
                } else if is_storage_buffer {
                    DescriptorType::STORAGE_BUFFER
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
            is_bindless: false,
            name: "".to_string(),
        })
    }

    /// Return a nested HashMap, the first HashMap is key'd off of the descriptor set,
    /// the second HashMap is key'd off of the descriptor index.
    pub fn get_descriptor_sets(&self) -> Result<HashMap<u32, HashMap<u32, DescriptorInfo>>> {
        let mut unique_sets = HashMap::new();
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
            .debugs
            .iter()
            .filter(|i| i.class.opcode == spirv::Op::Name)
            .map(|i| -> Result<(u32, String), ReflectError> {
                let element_type_id = get_operand_at!(i, Operand::IdRef, 0)?;
                let name = get_ref_operand_at!(i, Operand::LiteralString, 1)?;
                Ok((element_type_id, name.clone()))
            })
            .collect::<Result<HashMap<_, _>, _>>()?;

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
                    .or_insert_with(HashMap::<u32, DescriptorInfo>::new);

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

    pub fn disassemble(&self) -> String {
        use rspirv::binary::Disassemble;
        self.0.disassemble()
    }
}
