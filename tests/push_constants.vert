// https://github.com/Traverse-Research/rspirv-reflect/issues/29

#version 460

#extension GL_EXT_buffer_reference : require
#extension GL_EXT_buffer_reference2 : require

struct Mesh {
    vec4 position;
    vec2 uv;
};

layout(std430, buffer_reference, buffer_reference_align = 32) readonly buffer MeshBuffer
{
    Mesh mesh[];
};
layout(std430, buffer_reference) readonly buffer IndexBuffer
{
    uint index[];
};

layout(push_constant) uniform Registers
{
    MeshBuffer mesh_buffer;
    IndexBuffer index_buffer;
}
registers;

layout(location = 0) out vec2 uv;

void main()
{
    uint index = registers.index_buffer.index[gl_VertexIndex];
    Mesh mesh = registers.mesh_buffer.mesh[index];
    gl_Position = mesh.position;
    uv = mesh.uv;
}
