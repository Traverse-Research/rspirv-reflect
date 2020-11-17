struct TestType {
    float4 asdf;
};

StructuredBuffer<uint> g_input : register(t0, space0);
RWStructuredBuffer<uint> g_output : register(u1, space0);
ConstantBuffer<TestType> g_constant : register(b2, space0);
StructuredBuffer<uint> g_bindlessInput[] : register(t0, space1);
Texture2D<float4> g_texture2d : register(t0, space2);
RWTexture2D<uint> g_rwtexture2d : register(u0, space3);
RWTexture2D<uint> g_bindlessrwtexture2d[] : register(u0, space4);

SamplerState g_sampler : register(s0, space5);
ByteAddressBuffer g_byteAddressBuffer[] : register(t0, space6);
RWByteAddressBuffer g_rwbyteAddressBuffer : register(u0, space7);

static const uint s_constant = 34;

[numthreads(64, 1, 1)]
void main(int threadId: SV_DispatchThreadID)
{
    g_output[threadId] = s_constant
        + g_input[threadId]
        + g_constant.asdf.x
        + g_bindlessInput[threadId][threadId]
        + (uint)g_texture2d.Load(0, 0).x
        + g_rwtexture2d.Load(int2(0, 0)).x
        + g_bindlessrwtexture2d[threadId].Load(int2(0, 0)).x
        + (uint)g_texture2d.SampleLevel(g_sampler, int2(0, 0), 0.0).x
        + g_byteAddressBuffer[threadId].Load(0).x;
}