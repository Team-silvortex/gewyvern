namespace Leserpent.ControlPlane;

public static class BrowserSecurityHeaders
{
    public const string ContentSecurityPolicy =
        "default-src 'self'; "
        + "base-uri 'none'; "
        + "object-src 'none'; "
        + "frame-ancestors 'none'; "
        + "form-action 'self'; "
        + "script-src 'self'; "
        + "style-src 'self'; "
        + "img-src 'self' data:; "
        + "font-src 'self'; "
        + "connect-src 'self'; "
        + "frame-src http: https:; "
        + "worker-src 'none'";

    public static void Apply(HttpResponse response)
    {
        response.Headers["Content-Security-Policy"] = ContentSecurityPolicy;
        response.Headers["X-Frame-Options"] = "DENY";
        response.Headers["X-Content-Type-Options"] = "nosniff";
        response.Headers["Referrer-Policy"] = "no-referrer";
        response.Headers["Permissions-Policy"] =
            "camera=(), microphone=(), geolocation=(), payment=(), usb=()";
    }
}
