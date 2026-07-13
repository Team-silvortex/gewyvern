using Leserpent.ControlPlane;

namespace Leserpent;

public partial class Program
{
    private static void MapSessionEndpoints(WebApplication app)
    {
        app.MapGet("/v1/sessions", (RegistryService registry) =>
            Results.Ok(new SessionCollectionResponse(registry.ListSessions())));

        app.MapGet("/v1/sessions/{id}", (string id, RegistryService registry) =>
        {
            var session = registry.GetSession(id);
            return session is null ? Results.NotFound(new ApiErrorResponse("session_not_found", SessionId: id)) : Results.Ok(session);
        });

        app.MapPost("/v1/sessions", (SessionCreateRequest request, RegistryService registry) =>
        {
            if (string.IsNullOrWhiteSpace(request.RuntimeId) ||
                string.IsNullOrWhiteSpace(request.PipelineKind) ||
                string.IsNullOrWhiteSpace(request.RequestedBy))
            {
                return Results.BadRequest(new ApiErrorResponse(
                    "invalid_session_request",
                    "runtimeId, pipelineKind, and requestedBy are required"));
            }

            var result = registry.CreateSession(request);
            if (result.RuntimeMissing is not null)
            {
                return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: result.RuntimeMissing));
            }

            if (result.Rejections.Count > 0)
            {
                return Results.BadRequest(new ApiErrorResponse(
                    "capability_requirements_not_satisfied",
                    Rejections: result.Rejections));
            }

            return Results.Ok(result.Session);
        });

        app.MapPost("/v1/sessions/{id}/stop", (string id, SessionStopRequest request, RegistryService registry) =>
        {
            if (string.IsNullOrWhiteSpace(request.RequestedBy))
            {
                return Results.BadRequest(new ApiErrorResponse("invalid_stop_request", "requestedBy is required"));
            }

            var session = registry.StopSession(id);
            return session is null ? Results.NotFound(new ApiErrorResponse("session_not_found", SessionId: id)) : Results.Ok(session);
        });
    }
}
