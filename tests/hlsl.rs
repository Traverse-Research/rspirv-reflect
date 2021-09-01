use rspirv_reflect::*;

#[test]
fn hlsl_bindings() {
    let spirv = include_bytes!("shader_cs-hlsl.spv");

    let reflect = Reflection::new_from_spirv(spirv)
        .expect("Failed to create reflection module from spirv code");

    println!("{}", reflect.disassemble());

    let sets = reflect
        .get_descriptor_sets()
        .expect("Failed to extract descriptor sets");

    dbg!(&sets);

    let set0 = &sets[&0];
    let set1 = &sets[&1];
    let set2 = &sets[&2];
    let set3 = &sets[&3];
    let set4 = &sets[&4];
    let set5 = &sets[&5];
    let set6 = &sets[&6];
    let set7 = &sets[&7];
    let set8 = &sets[&8];

    assert_eq!(
        set0[&0],
        DescriptorInfo {
            name: "g_input".to_string(),
            ty: DescriptorType::STORAGE_BUFFER,
            is_bindless: false
        }
    );

    assert_eq!(
        set0[&1],
        DescriptorInfo {
            name: "g_output".to_string(),
            ty: DescriptorType::STORAGE_BUFFER,
            is_bindless: false
        }
    );

    assert_eq!(
        set0[&2],
        DescriptorInfo {
            name: "g_constant".to_string(),
            ty: DescriptorType::UNIFORM_BUFFER,
            is_bindless: false
        }
    );

    assert_eq!(
        set1[&0],
        DescriptorInfo {
            name: "g_bindlessInput".to_string(),
            ty: DescriptorType::STORAGE_BUFFER,
            is_bindless: true
        }
    );

    assert_eq!(
        set2[&0],
        DescriptorInfo {
            name: "g_texture2d".to_string(),
            ty: DescriptorType::SAMPLED_IMAGE,
            is_bindless: false
        }
    );

    assert_eq!(
        set3[&0],
        DescriptorInfo {
            name: "g_rwtexture2d".to_string(),
            ty: DescriptorType::STORAGE_IMAGE,
            is_bindless: false
        }
    );

    assert_eq!(
        set4[&0],
        DescriptorInfo {
            name: "g_bindlessrwtexture2d".to_string(),
            ty: DescriptorType::STORAGE_IMAGE,
            is_bindless: true
        }
    );

    assert_eq!(
        set5[&0],
        DescriptorInfo {
            name: "g_sampler".to_string(),
            ty: DescriptorType::SAMPLER,
            is_bindless: false
        }
    );

    assert_eq!(
        set6[&0],
        DescriptorInfo {
            name: "g_byteAddressBuffer".to_string(),
            ty: DescriptorType::STORAGE_BUFFER,
            is_bindless: true
        }
    );

    assert_eq!(
        set7[&0],
        DescriptorInfo {
            name: "g_rwbyteAddressBuffer".to_string(),
            ty: DescriptorType::STORAGE_BUFFER,
            is_bindless: false
        }
    );

    assert_eq!(
        set8[&0],
        DescriptorInfo {
            name: "g_inputArray".to_string(),
            ty: DescriptorType::STORAGE_BUFFER,
            is_bindless: false
        }
    );

    assert_eq!(
        set8[&1],
        DescriptorInfo {
            name: "g_arrayOfInputs".to_string(),
            ty: DescriptorType::STORAGE_BUFFER,
            is_bindless: false
        }
    );

    assert_eq!(
        set8[&6],
        DescriptorInfo {
            name: "g_bindlessInputArray".to_string(),
            ty: DescriptorType::STORAGE_BUFFER,
            is_bindless: true
        }
    );
}

#[test]
fn push_constants() {
    let spirv = include_bytes!("push_constant_tests_ps-hlsl.spv");
    let reflect = Reflection::new_from_spirv(spirv)
        .expect("Failed to create reflection module from spirv code");
    let range = reflect
        .get_push_constant_range()
        .expect("failed to extract push constants")
        .expect("defined push constants not detected");

    assert_eq!(range.size, 404);
}
