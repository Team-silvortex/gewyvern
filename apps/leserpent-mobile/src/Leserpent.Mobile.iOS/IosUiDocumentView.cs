using UIKit;

public sealed class IosUiDocumentView : UIView
{
    private readonly Func<MobileUiDocumentBinding, MobileUiNodeBinding, Task> invokeAction;
    private readonly UIStackView content;

    public IosUiDocumentView(
        Func<MobileUiDocumentBinding, MobileUiNodeBinding, Task> invokeAction)
    {
        this.invokeAction = invokeAction;
        content = new UIStackView
        {
            Axis = UILayoutConstraintAxis.Vertical,
            Spacing = 10,
            TranslatesAutoresizingMaskIntoConstraints = false,
        };
        AddSubview(content);
        NSLayoutConstraint.ActivateConstraints([
            content.LeadingAnchor.ConstraintEqualTo(LeadingAnchor),
            content.TrailingAnchor.ConstraintEqualTo(TrailingAnchor),
            content.TopAnchor.ConstraintEqualTo(TopAnchor),
            content.BottomAnchor.ConstraintEqualTo(BottomAnchor),
        ]);
    }

    public void Mount(
        MobileUiDocumentBinding document,
        RemoteMutationAvailability availability,
        bool busy,
        int runtimeColumns)
    {
        Clear();
        var heading = document.Root.Children.FirstOrDefault(node =>
            node.Kind == UiNodeKind.Heading)?.Text ?? "Remote runtimes";
        content.AddArrangedSubview(Text(
            heading,
            UIFontTextStyle.Title2,
            gold: true,
            bold: true));
        if (document.Root.Kind == UiNodeKind.Column)
        {
            MountFleet(document, availability, busy, runtimeColumns);
            return;
        }
        foreach (var node in document.Root.Children.Where(node => node.Kind != UiNodeKind.Heading))
        {
            content.AddArrangedSubview(RenderNode(document, node, availability, busy));
        }
    }

    public void MountFailure(string message)
    {
        Clear();
        content.AddArrangedSubview(Text(message, UIFontTextStyle.Body));
    }

    private void MountFleet(
        MobileUiDocumentBinding document,
        RemoteMutationAvailability availability,
        bool busy,
        int runtimeColumns)
    {
        var cards = document.Root.Children
            .Where(node => node.Kind == UiNodeKind.RuntimeCard)
            .ToArray();
        foreach (var node in document.Root.Children.Where(node =>
                     node.Kind is not (UiNodeKind.Heading or UiNodeKind.RuntimeCard)
                     && node.Id != "remote-state"))
        {
            content.AddArrangedSubview(RenderNode(document, node, availability, busy));
        }
        if (cards.Length == 0)
        {
            content.AddArrangedSubview(Text(
                "No runtime projection available.",
                UIFontTextStyle.Body));
            return;
        }
        var columnCount = Math.Clamp(runtimeColumns, 1, 2);
        var columns = Enumerable.Range(0, columnCount)
            .Select(_ => new UIStackView
            {
                Axis = UILayoutConstraintAxis.Vertical,
                Spacing = 10,
            })
            .ToArray();
        for (var index = 0; index < cards.Length; index++)
        {
            columns[index % columns.Length].AddArrangedSubview(
                RenderNode(document, cards[index], availability, busy));
        }
        var host = new UIStackView(columns)
        {
            Axis = columnCount == 1
                ? UILayoutConstraintAxis.Vertical
                : UILayoutConstraintAxis.Horizontal,
            Distribution = columnCount == 1
                ? UIStackViewDistribution.Fill
                : UIStackViewDistribution.FillEqually,
            Alignment = UIStackViewAlignment.Fill,
            Spacing = 10,
        };
        content.AddArrangedSubview(host);
    }

    private UIView RenderNode(
        MobileUiDocumentBinding document,
        MobileUiNodeBinding node,
        RemoteMutationAvailability availability,
        bool busy) => node.Kind switch
        {
            UiNodeKind.RuntimeCard
                or UiNodeKind.Section
                or UiNodeKind.Column
                or UiNodeKind.RuntimeWorkspace =>
                RenderContainer(document, node, availability, busy),
            UiNodeKind.Action => RenderAction(document, node, availability, busy),
            UiNodeKind.Heading => Text(
                node.Text ?? node.AccessibleName ?? "Unavailable",
                UIFontTextStyle.Headline,
                gold: true,
                bold: true,
                node: node),
            UiNodeKind.LogEntry => Text(
                node.Text ?? node.AccessibleName ?? "Unavailable",
                UIFontTextStyle.Caption1,
                error: node.Text?.StartsWith("[ERROR]", StringComparison.Ordinal) == true,
                node: node),
            _ => Text(
                node.Text ?? node.AccessibleName ?? "Unavailable",
                UIFontTextStyle.Body,
                node: node),
        };

    private UIView RenderContainer(
        MobileUiDocumentBinding document,
        MobileUiNodeBinding node,
        RemoteMutationAvailability availability,
        bool busy)
    {
        var stack = new UIStackView
        {
            Axis = UILayoutConstraintAxis.Vertical,
            Spacing = 8,
            LayoutMarginsRelativeArrangement = true,
            LayoutMargins = new UIEdgeInsets(14, 14, 14, 14),
            BackgroundColor = UIColor.FromRGB(32, 28, 21),
            AccessibilityIdentifier = node.Id,
            AccessibilityLabel = Safe(node.AccessibleName ?? node.Text ?? node.Id),
        };
        stack.Layer.CornerRadius = 12;
        if (node.Text is { } title)
        {
            stack.AddArrangedSubview(Text(
                title,
                UIFontTextStyle.Headline,
                gold: true,
                bold: true,
                node: node));
        }
        foreach (var child in node.Children)
        {
            stack.AddArrangedSubview(RenderNode(document, child, availability, busy));
        }
        return stack;
    }

    private UIButton RenderAction(
        MobileUiDocumentBinding document,
        MobileUiNodeBinding node,
        RemoteMutationAvailability availability,
        bool busy)
    {
        var enabled = !busy && ActionEnabled(node.ActionKind, availability);
        var reason = ActionUnavailableReason(node.ActionKind, availability);
        var button = new UIButton(UIButtonType.System);
        button.SetTitle(
            Safe(node.Text ?? node.AccessibleName ?? "Action"),
            UIControlState.Normal);
        button.Enabled = enabled;
        button.BackgroundColor = enabled
            ? UIColor.FromRGB(255, 178, 41)
            : UIColor.FromRGB(90, 81, 66);
        button.SetTitleColor(
            enabled ? UIColor.FromRGB(17, 16, 13) : UIColor.FromRGB(211, 200, 178),
            UIControlState.Normal);
        button.TitleLabel!.Font = UIFont.GetPreferredFontForTextStyle(UIFontTextStyle.Headline)!;
        button.TitleLabel.AdjustsFontForContentSizeCategory = true;
        button.TitleLabel.Lines = 0;
        button.TitleLabel.LineBreakMode = UILineBreakMode.WordWrap;
        button.TitleLabel.TextAlignment = UITextAlignment.Center;
        button.Layer.CornerRadius = 10;
        button.HeightAnchor.ConstraintGreaterThanOrEqualTo(
            MobileLayoutPolicy.MinimumTouchTargetDp).Active = true;
        button.AccessibilityIdentifier = node.Id;
        button.AccessibilityLabel = Safe(node.AccessibleName ?? node.Text ?? "Action");
        button.AccessibilityHint = Safe(string.Join(". ", new[]
        {
            node.AccessibleDescription,
            enabled ? null : reason,
        }.Where(value => !string.IsNullOrWhiteSpace(value))));
        button.TouchUpInside += async (_, _) => await invokeAction(document, node);
        return button;
    }

    private static UILabel Text(
        string value,
        UIFontTextStyle style,
        bool gold = false,
        bool bold = false,
        bool error = false,
        MobileUiNodeBinding? node = null)
    {
        var descriptor = UIFontDescriptor.GetPreferredDescriptorForTextStyle(style);
        var font = UIFont.FromDescriptor(
            bold
                ? descriptor.CreateWithTraits(UIFontDescriptorSymbolicTraits.Bold)
                : descriptor,
            0)!;
        return new UILabel
        {
            Text = Safe(value),
            TextColor = error
                ? UIColor.FromRGB(255, 138, 101)
                : gold
                    ? UIColor.FromRGB(244, 201, 93)
                    : UIColor.FromRGB(233, 225, 208),
            Font = font,
            Lines = 0,
            AdjustsFontForContentSizeCategory = true,
            AccessibilityIdentifier = node?.Id,
            AccessibilityLabel = Safe(node?.AccessibleName ?? value),
        };
    }

    private void Clear()
    {
        foreach (var view in content.ArrangedSubviews)
        {
            content.RemoveArrangedSubview(view);
            view.RemoveFromSuperview();
            view.Dispose();
        }
    }

    private static bool ActionEnabled(
        ActionKind? kind,
        RemoteMutationAvailability availability) => kind switch
        {
            ActionKind.RuntimeInspect => availability.InspectEnabled,
            ActionKind.RuntimeRefresh
                or ActionKind.RuntimeCapabilitiesRefresh
                or ActionKind.RuntimeDeploy => availability.MutationsEnabled,
            _ => false,
        };

    private static string? ActionUnavailableReason(
        ActionKind? kind,
        RemoteMutationAvailability availability) => kind switch
        {
            ActionKind.RuntimeInspect => availability.InspectUnavailableReason,
            ActionKind.RuntimeRefresh
                or ActionKind.RuntimeCapabilitiesRefresh
                or ActionKind.RuntimeDeploy => availability.MutationUnavailableReason,
            _ => "Unsupported mobile action",
        };

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(256)
        .ToArray());
}
