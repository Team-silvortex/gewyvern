namespace Leserpent.ControlPlane;

public static class LanguagePackRequestPolicy
{
    public const string ErrorCode = "language_pack_credentials_forbidden";

    public static bool TryAccept(HttpRequest request, out ApiErrorResponse payload)
    {
        payload = new ApiErrorResponse("none");
        if (!request.Path.StartsWithSegments("/language-packs"))
        {
            return true;
        }

        if (!request.Headers.ContainsKey("Authorization")
            && !request.Headers.ContainsKey(ControlPlaneSecurityPolicy.AdminTokenHeader))
        {
            return true;
        }

        payload = new ApiErrorResponse(
            ErrorCode,
            "public language-pack requests must not carry credentials");
        return false;
    }
}
