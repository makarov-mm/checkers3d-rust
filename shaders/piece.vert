// A draughtsman; in the reflection pass, mirrored in the board.

layout(location = 0) in vec3 aPosition;
layout(location = 1) in vec3 aNormal;
layout(location = 2) in float aOcclusion;

uniform mat4 uModel;

out vec3 vWorld;
out vec3 vNormal;
out vec3 vLocal;
out float vOcclusion;

void main()
{
    vec4 world = uModel * vec4(aPosition, 1.0);
    vWorld = world.xyz;
    vNormal = mat3(uModel) * aNormal;
    vLocal = aPosition;
    vOcclusion = aOcclusion;

    if (uMirror.y > 0.5) {
        gl_ClipDistance[0] = world.y - uMirror.x;
        world.y = 2.0 * uMirror.x - world.y;
    }
    else {
        gl_ClipDistance[0] = 1.0;
    }
    gl_Position = uViewProjection * world;
}
