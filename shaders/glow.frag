// The hint arrow: light flowing from the piece to its square.

in vec3 vWorld;
in vec3 vNormal;
in vec2 vUv; // x along the arrow from 0 to 1, y across it

uniform vec4 uColor; // rgb, alpha

layout(location = 0) out vec4 fragColor;

void main()
{
    float time = uCameraPosition.w;
    float flow = 0.65 + 0.35 * sin((vUv.x * 7.0 - time * 5.0));
    float edge = 1.0 - smoothstep(0.55, 1.0, abs(vUv.y));
    vec3 color = uColor.rgb * flow * (0.6 + 0.4 * edge);
    fragColor = vec4(color, uColor.a * (0.55 + 0.45 * edge));
}
