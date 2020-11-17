struct PSInput {
    float4 color : COLOR;
};

struct PushConstant {
    uint a;
    float b;
    bool c;
    uint64_t d;
    float64_t e[6];
    float4x4 f[5];
    vector<int, 3> g;
};

[[vk::push_constant]] PushConstant p;

float4 main(PSInput input) : SV_TARGET { return input.color * p.a; }