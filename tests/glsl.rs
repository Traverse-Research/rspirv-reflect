use rspirv_reflect::*;

#[test]
fn glsl_bindings() {
    let spirv = include_bytes!("shader-glsl.spv");

    let reflect = Reflection::new_from_spirv(spirv)
        .expect("Failed to create reflection module from spirv code");

    println!("{}", reflect.disassemble());

    let sets = reflect
        .get_descriptor_sets()
        .expect("Failed to extract descriptor sets");

    dbg!(&sets);

    // assert_eq!(
    //     sets[&0][&0],
    //     DescriptorInfo {
    //         name: "g_input".to_string(),
    //         ty: DescriptorType::STORAGE_BUFFER,
    //         is_bindless: false
    //     }
    // );

    // assert_eq!(
    //     sets[&0][&1],
    //     DescriptorInfo {
    //         name: "g_output".to_string(),
    //         ty: DescriptorType::STORAGE_BUFFER,
    //         is_bindless: false
    //     }
    // );

    // assert_eq!(
    //     sets[&0][&2],
    //     DescriptorInfo {
    //         name: "g_constant".to_string(),
    //         ty: DescriptorType::UNIFORM_BUFFER,
    //         is_bindless: false
    //     }
    // );

    // assert_eq!(
    //     sets[&1][&0],
    //     DescriptorInfo {
    //         name: "g_bindlessInput".to_string(),
    //         ty: DescriptorType::STORAGE_BUFFER,
    //         is_bindless: true
    //     }
    // );

    assert_eq!(
        sets[&2][&0],
        DescriptorInfo {
            name: "g_rimage2d".to_string(),
            ty: DescriptorType::STORAGE_IMAGE,
            is_bindless: false
        }
    );

    assert_eq!(
        sets[&2][&1],
        DescriptorInfo {
            name: "g_wimage2d".to_string(),
            ty: DescriptorType::STORAGE_IMAGE,
            is_bindless: false
        }
    );

    assert_eq!(
        sets[&2][&2],
        DescriptorInfo {
            name: "g_rwimage2d".to_string(),
            ty: DescriptorType::STORAGE_IMAGE,
            is_bindless: false
        }
    );

    assert_eq!(
        sets[&3][&0],
        DescriptorInfo {
            name: "g_texture2d".to_string(),
            ty: DescriptorType::SAMPLED_IMAGE,
            is_bindless: false
        }
    );

    assert_eq!(
        sets[&4][&0],
        DescriptorInfo {
            name: "g_bindlessrwimage2d".to_string(),
            ty: DescriptorType::STORAGE_IMAGE,
            is_bindless: true
        }
    );

    assert_eq!(
        sets[&4][&1],
        DescriptorInfo {
            name: "g_bindlesstexture2d".to_string(),
            ty: DescriptorType::SAMPLED_IMAGE,
            is_bindless: true
        }
    );

    assert_eq!(
        sets[&5][&0],
        DescriptorInfo {
            name: "g_samplerimage2d".to_string(),
            ty: DescriptorType::COMBINED_IMAGE_SAMPLER,
            is_bindless: false
        }
    );

    assert_eq!(
        sets[&6][&0],
        DescriptorInfo {
            name: "g_imagebuffer".to_string(),
            ty: DescriptorType::STORAGE_TEXEL_BUFFER,
            is_bindless: false
        }
    );
    assert_eq!(
        sets[&6][&1],
        DescriptorInfo {
            name: "g_samplerbuffer".to_string(),
            ty: DescriptorType::UNIFORM_TEXEL_BUFFER,
            is_bindless: false
        }
    );

    // assert_eq!(
    //     sets[&6][&0],
    //     DescriptorInfo {
    //         name: "g_byteAddressBuffer".to_string(),
    //         ty: DescriptorType::STORAGE_BUFFER,
    //         is_bindless: true
    //     }
    // );
}
