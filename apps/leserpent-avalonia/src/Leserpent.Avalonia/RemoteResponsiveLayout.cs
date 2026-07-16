internal enum RemoteLayoutDensity
{
    Compact,
    Wide,
}

internal static class RemoteResponsiveLayout
{
    public const double CompactBreakpoint = 780;

    public static RemoteLayoutDensity Select(double width) => width < CompactBreakpoint
        ? RemoteLayoutDensity.Compact
        : RemoteLayoutDensity.Wide;

    public static void VerifyContract()
    {
        if (Select(640) != RemoteLayoutDensity.Compact
            || Select(CompactBreakpoint - 1) != RemoteLayoutDensity.Compact
            || Select(CompactBreakpoint) != RemoteLayoutDensity.Wide
            || Select(1080) != RemoteLayoutDensity.Wide)
        {
            throw new InvalidDataException("remote responsive layout breakpoint is invalid");
        }
    }
}
