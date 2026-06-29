using Leserpent.ControlPlane;

namespace Leserpent;

public partial class Program
{
    private static void MapSessionEndpoints(WebApplication app)
    {
        app.MapGet("/v1/sessions", (RegistryService registry) =>
            Results.Ok(new
            {
                sessions = registry.ListSessions(),
            }));

        app.MapGet("/v1/sessions/{id}", (string id, RegistryService registry) =>
        {
            var session = registry.GetSession(id);
            return session is null ? Results.NotFound(new { error = "session_not_found", sessionId = id }) : Results.Ok(session);
        });

        app.MapPost("/v1/sessions", (SessionCreateRequest request, RegistryService registry) =>
        {
            if (string.IsNullOrWhiteSpace(request.RuntimeId) ||
                string.IsNullOrWhiteSpace(request.PipelineKind) ||
                string.IsNullOrWhiteSpace(request.RequestedBy))
            {
                return Results.BadRequest(new
                {
                    error = "invalid_session_request",
                    reason = "runtimeId, pipelineKind, and requestedBy are required",
                });
            }

            var result = registry.CreateSession(request);
            if (result.RuntimeMissing is not null)
            {
                return Results.NotFound(new
                {
                    error = "runtime_not_found",
                    runtimeId = result.RuntimeMissing,
                });
            }

            if (result.Rejections.Count > 0)
            {
                return Results.BadRequest(new
                {
                    error = "capability_requirements_not_satisfied",
                    rejections = result.Rejections,
                });
            }

            return Results.Ok(result.Session);
        });

        app.MapPost("/v1/sessions/{id}/stop", (string id, SessionStopRequest request, RegistryService registry) =>
        {
            if (string.IsNullOrWhiteSpace(request.RequestedBy))
            {
                return Results.BadRequest(new
                {
                    error = "invalid_stop_request",
                    reason = "requestedBy is required",
                });
            }

            var session = registry.StopSession(id);
            return session is null ? Results.NotFound(new { error = "session_not_found", sessionId = id }) : Results.Ok(session);
        });
    }
}
