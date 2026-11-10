// The board, its frame and the table: static geometry in world space.

layout(location = 0) in vec3 aPosition;
layout(location = 1) in vec3 aNormal;
layout(location = 2) in vec2 aUv;

out vec3 vWorld;
out vec3 vNormal;
out vec2 vUv;

void main()
{
    vWorld = aPosition;
    vNormal = aNormal;
    vUv = aUv;
    gl_Position = uViewProjection * vec4(aPosition, 1.0);
}
