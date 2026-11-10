// A draughtsman: turned wood, stone, acrylic or metal, with a crown inlaid on top of a king, a selection
// glow and a dissolve.

in vec3 vWorld;
in vec3 vNormal;
in vec3 vLocal;
in float vOcclusion;

uniform mat4 uModel;
uniform vec3 uAlbedo;
uniform vec4 uSurface;  // roughness, metallic, clearcoat, clearcoat roughness
uniform vec4 uStyle;    // x: subsurface, y: pattern strength, z: pattern (0 none, 1 wood, 2 turned metal, 3 stone), w: height of the ground
uniform vec4 uGlow;     // rgb, strength: the selection
uniform vec4 uTrim;     // rgb, strength: a glowing ring round the side
uniform vec4 uDissolve; // x: 0 solid to 1 gone, yzw: edge color
uniform vec4 uCrown;    // x: the crown on top, y: a seed for the grain, z: ribs round the side

layout(location = 0) out vec4 fragColor;

const float DiscRadius = 0.4;
const float DiscHeight = 0.15;

// A crown in the plane, about 2 wide, its top towards +y; negative inside.
float Crown(vec2 p)
{
    float x = abs(p.x);
    // Three points: the middle one tallest.
    float top = x < 0.3 ? mix(0.78, 0.12, x / 0.3) : mix(0.12, 0.6, (x - 0.3) / 0.3);
    float body = max(max(x - 0.62, p.y - top), -0.3 - p.y);
    float band = max(abs(p.x) - 0.7, abs(p.y + 0.43) - 0.12);
    float balls = min(length(p - vec2(0.0, 0.86)) - 0.1, length(vec2(x, p.y) - vec2(0.6, 0.68)) - 0.085);
    return min(min(body, band), balls);
}

void main()
{
    // Measured before anything is discarded, while the neighbouring pixels still run.
    vec3 N = normalize(vNormal);
    float spread = NormalSpread(N);
    vec3 localPixel = fwidth(vLocal);
    float radiusPixel = max(fwidth(length(vLocal.xz)), 1e-5);

    vec3 emissive = vec3(0.0);
    if (uDissolve.x > 0.0) {
        float n = Fbm(vLocal * 7.0 + 3.0 + uCrown.y * 5.0);
        float threshold = uDissolve.x * 1.2 - 0.1;
        if (n < threshold)
            discard;
        emissive += uDissolve.yzw * (1.0 - smoothstep(0.0, 0.06, n - threshold)) * 8.0;
    }

    if (!gl_FrontFacing)
        N = -N;
    vec3 V = normalize(uEye.xyz - vWorld);

    Material m = DefaultMaterial(uAlbedo, uSurface.x);
    m.metallic = uSurface.y;
    m.clearcoat = uSurface.z;
    m.clearcoatRoughness = uSurface.w;
    m.subsurface = uStyle.x;

    float radius = length(vLocal.xz);
    float angle = atan(vLocal.z, vLocal.x);
    bool side = radius > DiscRadius - 0.012 && vLocal.y > 0.03 && vLocal.y < DiscHeight - 0.034;
    int pattern = int(uStyle.z + 0.5);
    if (pattern == 1) {
        // Wood cut along the grain: straight lines across the top, rings on the side.
        vec3 q = vLocal + vec3(uCrown.y * 3.1, 0.0, uCrown.y * 1.7);
        float warp = Fbm(q * vec3(2.0, 5.0, 5.0)) * 0.6;
        float rings = fract(length(q.yz + vec2(0.9 + uCrown.y, -0.7)) * 24.0 + warp);
        float late = smoothstep(0.0, 0.12, rings) * (1.0 - smoothstep(0.3, 0.65, rings));
        late = mix(late, 0.3, smoothstep(0.01, 0.03, length(localPixel)));
        float fibers = mix(Noise(vec2(q.x * 30.0, length(q.yz) * 400.0)), 0.5, smoothstep(0.0015, 0.005, length(localPixel)));
        m.albedo *= 1.0 - uStyle.y * (0.24 * late + 0.1 * fibers);
    }
    else if (pattern == 2) {
        // Turned metal: fine circular marks on the top.
        float marks = mix(Noise(vec2(radius * 900.0, angle * 3.0)), 0.5, smoothstep(0.0005, 0.0015, length(localPixel.xz)));
        m.roughness = clamp(m.roughness + (marks - 0.5) * 0.12 * uStyle.y, 0.03, 1.0);
    }
    else if (pattern == 3) {
        // Veined stone.
        vec3 q = vLocal + uCrown.y * 7.0;
        float warp = Fbm(q * 3.5 + 11.0);
        float veins = abs(sin((q.x * 2.0 + q.y * 3.0 + q.z + warp * 3.0) * 4.0));
        float vein = 1.0 - smoothstep(0.0, 0.08, veins);
        m.albedo = mix(m.albedo, m.albedo * 0.55 + vec3(0.12), vein * uStyle.y);
        m.albedo *= 0.94 + 0.12 * Fbm(q * 22.0);
    }

    // Ribs round the side, for the acrylic pieces, fading where they would be finer than a pixel.
    if (uCrown.z > 0.0) {
        float ribs = sin(angle * 72.0);
        float fade = 1.0 - smoothstep(0.01, 0.02, length(localPixel.xz));
        vec3 tangent = normalize(mat3(uModel) * vec3(-sin(angle), 0.0, cos(angle)));
        if (side)
            N = normalize(N + tangent * ribs * 0.35 * uCrown.z * fade);
    }

    // The crown, inlaid in the middle of the top of a king.
    float crownCover = 0.0;
    if (uCrown.x > 0.0) {
        vec2 p = vec2(vLocal.x, -vLocal.z) / 0.15;
        float d = Crown(p);
        float aa = max(length(localPixel.xz) / 0.15, 1e-4);
        crownCover = (1.0 - smoothstep(-aa, aa, d)) * step(DiscHeight - 0.03, vLocal.y) * uCrown.x;
    }
    if (crownCover > 0.0) {
        bool neon = uTrim.a > 0.0;
        vec3 inlay = neon ? uTrim.rgb : vec3(1.0, 0.74, 0.32);
        m.albedo = mix(m.albedo, inlay, crownCover);
        m.metallic = mix(m.metallic, neon ? 0.0 : 1.0, crownCover);
        m.roughness = mix(m.roughness, 0.24, crownCover);
        m.subsurface *= 1.0 - crownCover;
        emissive += inlay * crownCover * (neon ? uTrim.a * 0.5 : 0.08);
    }

    // Cloth under the bottom.
    if (vLocal.y < 0.004 && N.y < -0.8)
        m = DefaultMaterial(vec3(0.03, 0.09, 0.06), 1.0);

    // The ground darkens what faces it, near the bottom most.
    float height = vWorld.y - uStyle.w;
    float ground = mix(0.45, 1.0, smoothstep(0.0, 0.3, height));
    m.occlusion = vOcclusion * mix(1.0, ground, Saturate(0.65 - N.y * 0.65));

    // Neon in the groove round the top, and round the middle of the side.
    float topRing = LineCoverage(abs(radius - 0.305), 0.006, radiusPixel) * step(DiscHeight - 0.02, vLocal.y);
    float sideRing = LineCoverage(abs(vLocal.y - DiscHeight * 0.5), 0.008, max(localPixel.y, 1e-5)) * step(DiscRadius - 0.02, radius);
    m.emissive = emissive + uTrim.rgb * uTrim.a * (topRing + sideRing);

    m.roughness = WidenRoughness(m.roughness, spread);
    m.clearcoatRoughness = WidenRoughness(m.clearcoatRoughness, spread);
    float shadow = uEye.w > 0.5 ? 1.0 : Shadow(vWorld, N);
    vec3 color = Shade(m, vWorld, N, V, shadow);

    // The selection glow: a halo at the silhouette and a little light all over.
    float NoV = Saturate(dot(N, V));
    color += uGlow.rgb * uGlow.a * (0.06 + 1.6 * pow(1.0 - NoV, 3.0));

    if (uMirror.z > 0.5)
        fragColor = vec4(EncodeSrgb(Aces(color * uMirror.w)), 1.0); // drawn straight onto the interface
    else
        fragColor = vec4(ApplyFog(color, vWorld), 1.0);
}
