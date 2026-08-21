public enum MobileWidthClass
{
    Compact,
    Medium,
    Expanded,
}

public readonly record struct MobileSafeAreaInsets(
    double Left,
    double Top,
    double Right,
    double Bottom)
{
    public static MobileSafeAreaInsets Zero { get; } = new(0, 0, 0, 0);
}

public readonly record struct MobileResolvedInsets(
    int Left,
    int Top,
    int Right,
    int Bottom);

public readonly record struct MobileLayoutPlan(
    MobileWidthClass WidthClass,
    bool TwoPane,
    int RuntimeColumns,
    int ContentMaxWidthDp,
    int SectionSpacingDp,
    int MinimumTouchTargetDp,
    MobileResolvedInsets ContentInsets,
    MobileResolvedInsets ActionInsets);

public static class MobileLayoutPolicy
{
    public const double MediumBreakpointDp = 600;
    public const double ExpandedBreakpointDp = 840;
    public const double MinimumTwoPaneHeightDp = 480;
    public const int MinimumTouchTargetDp = 48;
    private const double MaximumViewportDp = 16384;
    private const double MaximumLayoutFontScale = 2.5;
    private const double MaximumSafeInsetDp = 256;

    public static MobileLayoutPlan Resolve(
        double widthDp,
        double heightDp,
        double fontScale,
        MobileSafeAreaInsets safeArea)
    {
        ValidateViewport(widthDp, nameof(widthDp));
        ValidateViewport(heightDp, nameof(heightDp));
        if (!double.IsFinite(fontScale) || fontScale <= 0)
        {
            throw new ArgumentOutOfRangeException(nameof(fontScale));
        }
        ValidateSafeArea(safeArea);

        var usableWidth = widthDp - safeArea.Left - safeArea.Right;
        var usableHeight = heightDp - safeArea.Top - safeArea.Bottom;
        if (usableWidth <= 0 || usableHeight <= 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(safeArea),
                "mobile safe area leaves no usable viewport");
        }

        // Larger accessibility text consumes horizontal space before a host chooses columns.
        var layoutFontScale = Math.Clamp(fontScale, 1, MaximumLayoutFontScale);
        var effectiveWidth = usableWidth / layoutFontScale;
        var effectiveHeight = usableHeight / layoutFontScale;
        var widthClass = effectiveWidth switch
        {
            < MediumBreakpointDp => MobileWidthClass.Compact,
            < ExpandedBreakpointDp => MobileWidthClass.Medium,
            _ => MobileWidthClass.Expanded,
        };
        var horizontalPadding = widthClass switch
        {
            MobileWidthClass.Compact => 16,
            MobileWidthClass.Medium => 20,
            MobileWidthClass.Expanded => 24,
            _ => throw new InvalidDataException("unsupported mobile width class"),
        };
        var verticalPadding = widthClass == MobileWidthClass.Compact ? 16 : 20;
        var sectionSpacing = widthClass == MobileWidthClass.Compact ? 16 : 20;
        var contentMaxWidth = widthClass switch
        {
            MobileWidthClass.Compact => 0,
            MobileWidthClass.Medium => 720,
            MobileWidthClass.Expanded => 1120,
            _ => throw new InvalidDataException("unsupported mobile width class"),
        };
        var contentOuterMargin = contentMaxWidth == 0
            ? 0
            : Math.Max(0, (widthDp - contentMaxWidth) / 2);
        var twoPane = widthClass == MobileWidthClass.Expanded
            && effectiveHeight >= MinimumTwoPaneHeightDp;
        return new MobileLayoutPlan(
            widthClass,
            twoPane,
            widthClass == MobileWidthClass.Compact ? 1 : 2,
            contentMaxWidth,
            sectionSpacing,
            MinimumTouchTargetDp,
            new MobileResolvedInsets(
                AddInset(
                    Math.Max(0, safeArea.Left - contentOuterMargin),
                    horizontalPadding),
                AddInset(safeArea.Top, verticalPadding),
                AddInset(
                    Math.Max(0, safeArea.Right - contentOuterMargin),
                    horizontalPadding),
                AddInset(safeArea.Bottom, sectionSpacing)),
            new MobileResolvedInsets(
                AddInset(safeArea.Left, horizontalPadding),
                10,
                AddInset(safeArea.Right, horizontalPadding),
                AddInset(safeArea.Bottom, 10)));
    }

    public static void VerifyContract()
    {
        var compact = Resolve(
            390,
            844,
            1,
            new MobileSafeAreaInsets(0, 47, 0, 34));
        var medium = Resolve(800, 1100, 1, MobileSafeAreaInsets.Zero);
        var expanded = Resolve(1200, 900, 1, MobileSafeAreaInsets.Zero);
        var shortLandscape = Resolve(1000, 430, 1, MobileSafeAreaInsets.Zero);
        var accessibility = Resolve(1000, 900, 2, MobileSafeAreaInsets.Zero);
        var extremeAccessibility = Resolve(1000, 900, 3, MobileSafeAreaInsets.Zero);
        var narrowWindow = Resolve(220, 180, 1, MobileSafeAreaInsets.Zero);
        var cutoutTablet = Resolve(
            1200,
            900,
            1,
            new MobileSafeAreaInsets(60, 0, 0, 0));
        if (compact is not
            {
                WidthClass: MobileWidthClass.Compact,
                TwoPane: false,
                RuntimeColumns: 1,
                ContentMaxWidthDp: 0,
                MinimumTouchTargetDp: MinimumTouchTargetDp,
                ContentInsets.Top: 63,
                ContentInsets.Bottom: 50,
                ActionInsets.Bottom: 44,
            }
            || medium is not
            {
                WidthClass: MobileWidthClass.Medium,
                TwoPane: false,
                RuntimeColumns: 2,
                ContentMaxWidthDp: 720,
            }
            || expanded is not
            {
                WidthClass: MobileWidthClass.Expanded,
                TwoPane: true,
                RuntimeColumns: 2,
                ContentMaxWidthDp: 1120,
            }
            || shortLandscape.TwoPane
            || accessibility.WidthClass != MobileWidthClass.Compact
            || accessibility.TwoPane
            || extremeAccessibility.WidthClass != MobileWidthClass.Compact
            || narrowWindow.WidthClass != MobileWidthClass.Compact
            || cutoutTablet.ContentInsets.Left != 44
            || !Rejects(() => Resolve(
                double.NaN,
                800,
                1,
                MobileSafeAreaInsets.Zero))
            || !Rejects(() => Resolve(390, 844, 0, MobileSafeAreaInsets.Zero))
            || !Rejects(() => Resolve(
                390,
                844,
                1,
                new MobileSafeAreaInsets(-1, 0, 0, 0))))
        {
            throw new InvalidDataException("mobile layout policy contract drifted");
        }
    }

    private static void ValidateViewport(double value, string name)
    {
        if (!double.IsFinite(value)
            || value <= 0
            || value > MaximumViewportDp)
        {
            throw new ArgumentOutOfRangeException(name);
        }
    }

    private static void ValidateSafeArea(MobileSafeAreaInsets safeArea)
    {
        ValidateSafeInset(safeArea.Left);
        ValidateSafeInset(safeArea.Top);
        ValidateSafeInset(safeArea.Right);
        ValidateSafeInset(safeArea.Bottom);
    }

    private static void ValidateSafeInset(double value)
    {
        if (!double.IsFinite(value) || value < 0 || value > MaximumSafeInsetDp)
        {
            throw new ArgumentOutOfRangeException("safeArea");
        }
    }

    private static int AddInset(double inset, int padding) =>
        checked((int)Math.Ceiling(inset) + padding);

    private static bool Rejects(Action action)
    {
        try
        {
            action();
            return false;
        }
        catch (ArgumentOutOfRangeException)
        {
            return true;
        }
    }
}
