use rspirv_reflect::*;

#[test]
fn bindings() {
    let spirv = include_bytes!("shader-glsl.spv");

    let reflect = Reflection::new_from_spirv(spirv)
        .expect("Failed to create reflection module from spirv code");

    println!("{}", reflect.disassemble());

    let sets = reflect
        .get_descriptor_sets()
        .expect("Failed to extract descriptor sets");

    dbg!(&sets);

    // WARNING: Array size is defined by how an "unbounded" x[] is *used*, not
    // by the fact that no size is specified in the declaration.  A descriptor
    // only receives BindingCount::Unbounded if it is used by a *nonuniform*
    // index.
    // Otherwise the size becomes the highest constant/uniform index plus one,
    // or 1 if it is not used.

    assert_eq!(
        sets[&0][&0],
        DescriptorInfo {
            name: "uniformBlock".to_string(),
            ty: DescriptorType::UNIFORM_BUFFER,
            binding_count: BindingCount::One
        }
    );

    assert_eq!(
        sets[&1][&0],
        DescriptorInfo {
            name: "g_rimage2d".to_string(),
            ty: DescriptorType::STORAGE_IMAGE,
            binding_count: BindingCount::One
        }
    );

    assert_eq!(
        sets[&1][&1],
        DescriptorInfo {
            name: "g_wimage2d".to_string(),
            ty: DescriptorType::STORAGE_IMAGE,
            binding_count: BindingCount::One
        }
    );

    assert_eq!(
        sets[&1][&2],
        DescriptorInfo {
            name: "g_rwimage2d".to_string(),
            ty: DescriptorType::STORAGE_IMAGE,
            binding_count: BindingCount::One
        }
    );

    assert_eq!(
        sets[&2][&0],
        DescriptorInfo {
            name: "g_texture2d".to_string(),
            ty: DescriptorType::SAMPLED_IMAGE,
            binding_count: BindingCount::One
        }
    );

    assert_eq!(
        sets[&3][&0],
        DescriptorInfo {
            name: "g_multiple_rwimage2d".to_string(),
            ty: DescriptorType::STORAGE_IMAGE,
            binding_count: BindingCount::StaticSized(10)
        }
    );

    assert_eq!(
        sets[&3][&1],
        DescriptorInfo {
            name: "g_multiple_texture2d".to_string(),
            ty: DescriptorType::SAMPLED_IMAGE,
            binding_count: BindingCount::StaticSized(10)
        }
    );

    assert_eq!(
        sets[&4][&0],
        DescriptorInfo {
            name: "g_bindless_rwimage2d".to_string(),
            ty: DescriptorType::STORAGE_IMAGE,
            binding_count: BindingCount::StaticSized(6)
        }
    );

    assert_eq!(
        sets[&4][&1],
        DescriptorInfo {
            name: "g_bindless_texture2d".to_string(),
            ty: DescriptorType::SAMPLED_IMAGE,
            binding_count: BindingCount::StaticSized(1)
        }
    );

    assert_eq!(
        sets[&4][&2],
        DescriptorInfo {
            name: "g_bindless_buffer".to_string(),
            ty: DescriptorType::UNIFORM_BUFFER,
            binding_count: BindingCount::StaticSized(11)
        }
    );
    assert_eq!(
        sets[&5][&0],
        DescriptorInfo {
            name: "g_samplerimage2d".to_string(),
            ty: DescriptorType::COMBINED_IMAGE_SAMPLER,
            binding_count: BindingCount::One
        }
    );

    assert_eq!(
        sets[&6][&0],
        DescriptorInfo {
            name: "g_imagebuffer".to_string(),
            ty: DescriptorType::STORAGE_TEXEL_BUFFER,
            binding_count: BindingCount::One
        }
    );
    assert_eq!(
        sets[&6][&1],
        DescriptorInfo {
            name: "g_samplerbuffer".to_string(),
            ty: DescriptorType::UNIFORM_TEXEL_BUFFER,
            binding_count: BindingCount::One
        }
    );

    assert_eq!(
        sets[&6][&2],
        DescriptorInfo {
            name: "g_storageBuffer".to_string(),
            ty: DescriptorType::STORAGE_BUFFER,
            binding_count: BindingCount::Unbounded
        }
    );

    assert_eq!(
        sets[&6][&3],
        DescriptorInfo {
            name: "bufferBlock".to_string(),
            ty: DescriptorType::STORAGE_BUFFER,
            binding_count: BindingCount::One
        }
    );
}

#[test]
fn push_constants() {
    let spirv = include_bytes!("push_constants-glsl.spv");

    let reflect = Reflection::new_from_spirv(spirv)
        .expect("Failed to create reflection module from spirv code");

    println!("{}", reflect.disassemble());

    let range = reflect
        .get_push_constant_range()
        .expect("failed to extract push constants")
        .expect("defined push constants not detected");

    dbg!(range);

    assert_eq!(
        range,
        PushConstantInfo {
            offset: 0,
            size: 16
        }
    )
}
