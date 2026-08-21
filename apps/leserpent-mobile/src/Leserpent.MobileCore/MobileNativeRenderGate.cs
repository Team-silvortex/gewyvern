public sealed class MobileNativeRenderGate
{
    private MobileUiDocumentBinding? document;
    private RemoteMutationAvailability? availability;
    private bool busy;
    private int runtimeColumns;

    public static MobileUiDocumentBinding RetainEquivalentPresentation(
        MobileUiDocumentBinding? current,
        MobileUiDocumentBinding candidate,
        string? ignoredNodeId = null)
    {
        ArgumentNullException.ThrowIfNull(candidate);
        return current?.HasSameNativePresentation(candidate, ignoredNodeId) == true
            ? current
            : candidate;
    }

    public bool ShouldRender(
        MobileUiDocumentBinding candidate,
        RemoteMutationAvailability candidateAvailability,
        bool candidateBusy,
        int candidateRuntimeColumns)
    {
        ArgumentNullException.ThrowIfNull(candidate);
        ArgumentNullException.ThrowIfNull(candidateAvailability);
        if (candidateRuntimeColumns is < 1 or > 2)
        {
            throw new ArgumentOutOfRangeException(nameof(candidateRuntimeColumns));
        }
        if (ReferenceEquals(document, candidate)
            && availability == candidateAvailability
            && busy == candidateBusy
            && runtimeColumns == candidateRuntimeColumns)
        {
            return false;
        }
        document = candidate;
        availability = candidateAvailability;
        busy = candidateBusy;
        runtimeColumns = candidateRuntimeColumns;
        return true;
    }

    public void Invalidate()
    {
        document = null;
        availability = null;
        runtimeColumns = 0;
    }
}
