// The depth of the pieces and the board as the key light sees them.

layout(location = 0) in vec3 aPosition;

uniform mat4 uModel;

out vec3 vLocal;

void main()
{
    vLocal = aPosition;
    gl_Position = uLightViewProjection * uModel * vec4(aPosition, 1.0);
}
