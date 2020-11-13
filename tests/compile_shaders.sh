#! /usr/bin/bash

DXC=${DXC:-dxc}
GLSLANG=${GLSLANG:-glslangValidator}

current_dir=$(dirname ${BASH_SOURCE[0]})

for hlsl in $current_dir/*.hlsl; do
    spirv=${hlsl%.hlsl}-hlsl.spv
    ${DXC} -E main -T cs_6_5 -spirv -fvk-use-scalar-layout $hlsl -Fo $spirv
done

for glsl in $current_dir/*.comp; do
    spirv=${glsl%.comp}-glsl.spv
    ${GLSLANG} -V $glsl -o $spirv
done