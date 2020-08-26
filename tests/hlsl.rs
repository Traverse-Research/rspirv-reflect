use rspirv_reflect::*;

#[test]
fn hlsl_bindings() {
    let spirv = include_bytes!("shader-hlsl.spv");

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

    assert_eq!(set0[&0].name, "g_input");
    assert_eq!(set0[&0].ty, DescriptorType::STORAGE_BUFFER);
    assert_eq!(set0[&0].is_bindless, false);

    assert_eq!(set0[&1].name, "g_output");
    assert_eq!(set0[&1].ty, DescriptorType::STORAGE_BUFFER);
    assert_eq!(set0[&1].is_bindless, false);

    assert_eq!(set0[&2].name, "g_constant");
    assert_eq!(set0[&2].ty, DescriptorType::UNIFORM_BUFFER);
    assert_eq!(set0[&2].is_bindless, false);

    assert_eq!(set1[&0].name, "g_bindlessInput");
    assert_eq!(set1[&0].ty, DescriptorType::STORAGE_BUFFER);
    assert_eq!(set1[&0].is_bindless, true);

    assert_eq!(set2[&0].name, "g_texture2d");
    assert_eq!(set2[&0].ty, DescriptorType::SAMPLED_IMAGE);
    assert_eq!(set2[&0].is_bindless, false);

    assert_eq!(set3[&0].name, "g_rwtexture2d");
    assert_eq!(set3[&0].ty, DescriptorType::STORAGE_IMAGE);
    assert_eq!(set3[&0].is_bindless, false);

    assert_eq!(set4[&0].name, "g_bindlessrwtexture2d");
    assert_eq!(set4[&0].ty, DescriptorType::STORAGE_IMAGE);
    assert_eq!(set4[&0].is_bindless, true);

    assert_eq!(set5[&0].name, "g_sampler");
    assert_eq!(set5[&0].ty, DescriptorType::SAMPLER);
    assert_eq!(set5[&0].is_bindless, false);

    assert_eq!(set6[&0].name, "g_byteAddressBuffer");
    assert_eq!(set6[&0].ty, DescriptorType::STORAGE_BUFFER);
    assert_eq!(set6[&0].is_bindless, true);
}
