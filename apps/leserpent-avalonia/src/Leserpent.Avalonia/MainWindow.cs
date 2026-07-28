using System.Text.Json;
using System.Text.Json.Nodes;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;

internal sealed class MainWindow : Window
{
    private readonly AvaloniaDocumentRenderer renderer;
    private readonly Task<PresentationAutomationResult> initialRealizedWait;
    private readonly Task<PresentationAutomationResult> initialRealizedWaitTimeout;
    private readonly TextBlock statusText = new()
    {
        Foreground = LeserpentTheme.Muted,
        FontSize = 13,
        Text = "No action selected",
    };

    public int RenderedNodeCount { get; }
    public int AppliedPatchOperations { get; }
    public int ReusedNodeCount { get; }
    public int VirtualizedHostCount { get; }
    public int ActiveVirtualizedHostCount => renderer.ActiveVirtualizedHostCount;
    public int InitialUnrealizedVirtualItemCount { get; }
    public int UnrealizedVirtualItemCount => renderer.UnrealizedVirtualItemCount;
    public int InitialUnrealizedNodeCount { get; }
    public int UnrealizedNodeCount => renderer.UnrealizedNodeCount;
    public bool InitialUnrealizedAssertionRejected { get; }
    public bool InitialRealizedWaitCompleted { get; private set; }
    public bool InitialRealizedWaitTimedOut { get; private set; }
    public int InitialDebuggerCancelButtonCount { get; }
    public int DebuggerCancelButtonCount => renderer.RealizedDebuggerCancelButtonCount;
    public int DisabledActionProbeCount { get; private set; }
    public AccessibilityAudit InitialAccessibility { get; }
    public AccessibilityAudit Accessibility => renderer.AuditAccessibility();
    public ulong Revision { get; }

    public MainWindow(RendererFixture fixture)
    {
        Width = 1080;
        Height = 760;
        MinWidth = 640;
        MinHeight = 480;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        renderer = new AvaloniaDocumentRenderer(OnActionInvoked);
        renderer.Mount(fixture.Previous);
        InitialDebuggerCancelButtonCount = renderer.RealizedDebuggerCancelButtonCount;
        InitialAccessibility = renderer.AuditAccessibility();
        renderer.Apply(fixture.Patch);
        RequireExpectedDocument(renderer.Document, fixture.Next);
        _ = renderer.AuditAccessibility();

        Revision = renderer.Document.Revision;
        RenderedNodeCount = renderer.NodeCount;
        AppliedPatchOperations = renderer.LastAppliedOperationCount;
        ReusedNodeCount = renderer.LastReusedNodeCount;
        VirtualizedHostCount = renderer.VirtualizedHostCount;
        InitialUnrealizedVirtualItemCount = renderer.UnrealizedVirtualItemCount;
        InitialUnrealizedNodeCount = renderer.UnrealizedNodeCount;
        var initialUnrealizedNodeId = renderer.FirstUnrealizedNodeId
            ?? throw new InvalidDataException(
                "realization assertion probe requires a pre-layout virtualized semantic node");
        var initialUnrealized = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertRealized,
            NodeId = initialUnrealizedNodeId,
        });
        InitialUnrealizedAssertionRejected = !initialUnrealized.Applied
            && initialUnrealized.FailureCode
                == PresentationAutomationFailureCode.TargetUnrealized;
        if (!InitialUnrealizedAssertionRejected)
        {
            throw new InvalidDataException(
                "Leselang realization assertion accepted a pre-layout unrealized target");
        }
        initialRealizedWait = renderer.ApplyPresentationAsync(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.WaitRealized,
            NodeId = initialUnrealizedNodeId,
            TimeoutMs = SemanticRenderer.WaitRealizedTimeoutMs,
        });
        var detachedRenderer = new AvaloniaDocumentRenderer(_ => { });
        detachedRenderer.Mount(fixture.Next);
        var detachedUnrealizedNodeId = detachedRenderer.FirstUnrealizedNodeId
            ?? throw new InvalidDataException(
                "realization wait timeout probe requires a detached unrealized node");
        initialRealizedWaitTimeout = detachedRenderer.ApplyPresentationAsync(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.WaitRealized,
                NodeId = detachedUnrealizedNodeId,
                TimeoutMs = SemanticRenderer.WaitRealizedTimeoutMs,
            });
        Title = $"Leserpent / revision {Revision}";
        Content = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("*,Auto"),
            Children =
            {
                new Border
                {
                    Padding = new Thickness(32, 28),
                    Child = renderer.Surface,
                },
                BuildStatusBar(),
            },
        };
    }

    public async Task CompleteInitialRealizedWaitProbeAsync()
    {
        var result = await initialRealizedWait;
        InitialRealizedWaitCompleted = result.Applied
            && result.FailureCode == PresentationAutomationFailureCode.None;
        if (!InitialRealizedWaitCompleted)
        {
            throw new InvalidDataException(
                "Leselang realization wait did not observe natural post-layout realization");
        }
        var timeoutResult = await initialRealizedWaitTimeout;
        InitialRealizedWaitTimedOut = !timeoutResult.Applied
            && timeoutResult.FailureCode == PresentationAutomationFailureCode.WaitTimedOut;
        if (!InitialRealizedWaitTimedOut)
        {
            throw new InvalidDataException(
                "Leselang realization wait did not reject a persistently unrealized target");
        }
    }

    public void ProbeActionAvailability()
    {
        ActionKind[] mutationKinds =
        [
            ActionKind.RuntimeRefresh,
            ActionKind.RuntimeCapabilitiesRefresh,
            ActionKind.RuntimeDeploy,
        ];
        var realizedActions = mutationKinds.Sum(renderer.RealizedActionCount);
        if (realizedActions == 0)
        {
            DisabledActionProbeCount = 0;
            _ = renderer.AuditAccessibility();
            return;
        }
        foreach (var kind in mutationKinds)
        {
            renderer.SetActionAvailability(
                kind,
                false,
                "Verification action is temporarily unavailable");
        }
        DisabledActionProbeCount = mutationKinds.Sum(
            renderer.RealizedDisabledActionCount);
        if (DisabledActionProbeCount != realizedActions)
        {
            throw new InvalidDataException(
                "Avalonia action availability did not disable every realized mutation action");
        }
        foreach (var kind in mutationKinds)
        {
            renderer.SetActionAvailability(kind, true, null);
        }
        if (mutationKinds.Sum(renderer.RealizedDisabledActionCount) != 0)
        {
            throw new InvalidDataException(
                "Avalonia action availability did not restore every realized mutation action");
        }
        _ = renderer.AuditAccessibility();
    }

    public string BeginFocusRetentionProbe()
    {
        var nodeId = renderer.FirstRealizedActionNodeId
            ?? throw new InvalidDataException("focus probe requires a realized action");
        var applied = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.Focus,
            NodeId = nodeId,
        });
        if (!applied.Applied
            || applied.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang presentation focus could not focus its action node");
        }
        var focused = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertFocused,
            NodeId = nodeId,
        });
        if (!focused.Applied
            || focused.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang focus assertion rejected the focused action");
        }
        var otherActionNodeId = FindOtherActionNodeId(renderer.Document.Root, nodeId)
            ?? throw new InvalidDataException("focus assertion probe requires another action");
        var unfocused = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertFocused,
            NodeId = otherActionNodeId,
        });
        if (unfocused.Applied
            || unfocused.FailureCode != PresentationAutomationFailureCode.TargetNotFocused
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang focus assertion accepted an unfocused action or changed focus");
        }
        var enabledNodeId = renderer.FirstRealizedActionNodeIdFor(ActionKind.RuntimeRefresh)
            ?? throw new InvalidDataException("enabled assertion probe requires a refresh action");
        var enabled = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertEnabled,
            NodeId = enabledNodeId,
        });
        if (!enabled.Applied
            || enabled.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang enabled assertion rejected an enabled action or changed focus");
        }
        renderer.SetActionAvailability(
            ActionKind.RuntimeRefresh,
            false,
            "Verification action is temporarily unavailable");
        var disabled = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertEnabled,
            NodeId = enabledNodeId,
        });
        renderer.SetActionAvailability(ActionKind.RuntimeRefresh, true, null);
        if (disabled.Applied
            || disabled.FailureCode != PresentationAutomationFailureCode.TargetNotEnabled
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang enabled assertion accepted a disabled action or changed focus");
        }
        var textNode = FindFirstTextNode(renderer.Document.Root)
            ?? throw new InvalidDataException("text assertion probe requires a text leaf");
        var expectedText = textNode.Text?.Fallback
            ?? throw new InvalidDataException("text assertion target has no semantic text");
        var textMatched = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertText,
            NodeId = textNode.Id,
            Expected = expectedText,
        });
        if (!textMatched.Applied
            || textMatched.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang text assertion rejected native display text or changed focus");
        }
        var textMismatch = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertText,
            NodeId = textNode.Id,
            Expected = $"{expectedText} mismatch",
        });
        if (textMismatch.Applied
            || textMismatch.FailureCode != PresentationAutomationFailureCode.TargetTextMismatch
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang text assertion accepted mismatched native display text or changed focus");
        }
        var expectedAccessibleName = textNode.Accessibility.Label?.Fallback
            ?? textNode.Text?.Fallback
            ?? textNode.Id;
        var accessibleNameMatched = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertAccessibleName,
            NodeId = textNode.Id,
            Expected = expectedAccessibleName,
        });
        if (!accessibleNameMatched.Applied
            || accessibleNameMatched.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang accessible name assertion rejected native automation metadata or changed focus");
        }
        var accessibleNameMismatch = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertAccessibleName,
            NodeId = textNode.Id,
            Expected = $"{expectedAccessibleName} mismatch",
        });
        if (accessibleNameMismatch.Applied
            || accessibleNameMismatch.FailureCode
                != PresentationAutomationFailureCode.TargetAccessibleNameMismatch
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang accessible name assertion accepted mismatched native metadata or changed focus");
        }
        var descriptionNode = FindFirstDescriptionNode(renderer.Document.Root)
            ?? throw new InvalidDataException(
                "accessible description probe requires explicit semantic metadata");
        var expectedAccessibleDescription = descriptionNode.Accessibility.Description!.Fallback;
        var accessibleDescriptionMatched = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertAccessibleDescription,
            NodeId = descriptionNode.Id,
            Expected = expectedAccessibleDescription,
        });
        if (!accessibleDescriptionMatched.Applied
            || accessibleDescriptionMatched.FailureCode
                != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang accessible description assertion rejected native automation metadata or changed focus");
        }
        var accessibleDescriptionMismatch = renderer.ApplyPresentation(
            new UiPresentationOperation
            {
                Kind = UiPresentationOperationKind.AssertAccessibleDescription,
                NodeId = descriptionNode.Id,
                Expected = $"{expectedAccessibleDescription} mismatch",
            });
        if (accessibleDescriptionMismatch.Applied
            || accessibleDescriptionMismatch.FailureCode
                != PresentationAutomationFailureCode.TargetAccessibleDescriptionMismatch
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang accessible description assertion accepted mismatched native metadata or changed focus");
        }
        var missing = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.Focus,
            NodeId = "missing-presentation-target",
        });
        if (missing.Applied
            || missing.FailureCode != PresentationAutomationFailureCode.UnknownTarget)
        {
            throw new InvalidDataException(
                "Leselang presentation focus accepted a missing target");
        }
        var nonActionNodeId = FindFirstNonActionNodeId(renderer.Document.Root)
            ?? throw new InvalidDataException("focus probe requires a non-action node");
        var unfocusable = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.Focus,
            NodeId = nonActionNodeId,
        });
        if (unfocusable.Applied
            || unfocusable.FailureCode != PresentationAutomationFailureCode.UnfocusableTarget)
        {
            throw new InvalidDataException(
                "Leselang presentation focus accepted an unfocusable target");
        }
        var scrolled = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.ScrollIntoView,
            NodeId = nonActionNodeId,
        });
        if (!scrolled.Applied
            || scrolled.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang presentation scroll failed or changed keyboard focus");
        }
        var missingScroll = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.ScrollIntoView,
            NodeId = "missing-presentation-target",
        });
        if (missingScroll.Applied
            || missingScroll.FailureCode != PresentationAutomationFailureCode.UnknownTarget)
        {
            throw new InvalidDataException(
                "Leselang presentation scroll accepted a missing target");
        }
        var visible = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertVisible,
            NodeId = nonActionNodeId,
        });
        if (!visible.Applied
            || visible.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang visibility assertion rejected a visible target or changed focus");
        }
        var realized = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertRealized,
            NodeId = nodeId,
        });
        if (!realized.Applied
            || realized.FailureCode != PresentationAutomationFailureCode.None
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang realization assertion rejected a realized target or changed focus");
        }
        if (!InitialUnrealizedAssertionRejected
            || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang realization assertion lost its pre-layout rejection evidence or changed focus");
        }
        renderer.Surface.IsVisible = false;
        var hidden = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.AssertVisible,
            NodeId = nonActionNodeId,
        });
        renderer.Surface.IsVisible = true;
        if (hidden.Applied
            || hidden.FailureCode != PresentationAutomationFailureCode.TargetNotVisible)
        {
            throw new InvalidDataException(
                "Leselang visibility assertion accepted a hidden target");
        }
        var refocused = renderer.ApplyPresentation(new UiPresentationOperation
        {
            Kind = UiPresentationOperationKind.Focus,
            NodeId = nodeId,
        });
        if (!refocused.Applied || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "Leselang presentation focus did not recover after the hidden assertion probe");
        }
        renderer.Mount(renderer.Document);
        if (!renderer.IsFocusRestorePending)
        {
            throw new InvalidDataException("focus restoration was not scheduled after mount");
        }
        return nodeId;
    }

    private static string? FindFirstNonActionNodeId(UiNode node)
    {
        if (node.Kind != UiNodeKind.Action)
        {
            return node.Id;
        }
        foreach (var child in node.Children)
        {
            if (FindFirstNonActionNodeId(child) is { } nodeId)
            {
                return nodeId;
            }
        }
        return null;
    }

    private static UiNode? FindFirstDescriptionNode(UiNode node)
    {
        if (node.Accessibility.Description is not null)
        {
            return node;
        }
        foreach (var child in node.Children)
        {
            if (FindFirstDescriptionNode(child) is { } described)
            {
                return described;
            }
        }
        return null;
    }

    private static UiNode? FindFirstTextNode(UiNode node)
    {
        if (node.Text is not null
            && node.Kind is UiNodeKind.Heading
                or UiNodeKind.Text
                or UiNodeKind.HistoryEntry
                or UiNodeKind.LogEntry
                or UiNodeKind.DebuggerFrame
                or UiNodeKind.Action)
        {
            return node;
        }
        foreach (var child in node.Children)
        {
            if (FindFirstTextNode(child) is { } match)
            {
                return match;
            }
        }
        return null;
    }

    private static string? FindOtherActionNodeId(UiNode node, string focusedNodeId)
    {
        if (node.Kind == UiNodeKind.Action
            && node.Action is not null
            && node.Id != focusedNodeId)
        {
            return node.Id;
        }
        foreach (var child in node.Children)
        {
            if (FindOtherActionNodeId(child, focusedNodeId) is { } nodeId)
            {
                return nodeId;
            }
        }
        return null;
    }

    public void CompleteFocusRetentionProbe(string nodeId)
    {
        if (renderer.IsFocusRestorePending || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "stable action focus was not restored after document mount");
        }
    }

    public void BeginPatchedFocusRetentionProbe(string nodeId)
    {
        if (!renderer.TryFocusNode(nodeId) || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException("patch focus probe could not focus its action node");
        }
        var source = FindNode(renderer.Document.Root, nodeId)
            ?? throw new InvalidDataException("patch focus probe action was not found");
        var revision = renderer.Document.Revision;
        renderer.Apply(new UiPatch
        {
            SchemaVersion = 1,
            FromRevision = revision,
            ToRevision = checked(revision + 1),
            Operations =
            [
                new UiPatchOperation
                {
                    Kind = PatchKind.Update,
                    Node = CloneShallow(source),
                },
            ],
        });
        if (!renderer.IsFocusRestorePending)
        {
            throw new InvalidDataException("focus restoration was not scheduled after patch");
        }
    }

    public void ProbeRemovedFocusTarget(string nodeId)
    {
        var payload = JsonSerializer.SerializeToUtf8Bytes(
            renderer.Document,
            RendererJsonContext.Default.UiDocument);
        var withoutFocusedAction = JsonSerializer.Deserialize(
            payload,
            RendererJsonContext.Default.UiDocument)
            ?? throw new InvalidDataException("focus probe document clone failed");
        if (!RemoveNode(withoutFocusedAction.Root, nodeId))
        {
            throw new InvalidDataException("focus probe could not remove its action node");
        }
        renderer.Mount(withoutFocusedAction);
        if (renderer.IsFocusRestorePending || renderer.FocusedNodeId is not null)
        {
            throw new InvalidDataException(
                "removed action focus was transferred to another control");
        }
    }

    private Border BuildStatusBar()
    {
        var revisionText = new TextBlock
        {
            Foreground = LeserpentTheme.Primary,
            FontSize = 12,
            FontWeight = FontWeight.SemiBold,
            Text = $"UI IR v1  /  rev {Revision}",
        };
        var bar = new Border
        {
            Background = LeserpentTheme.Panel,
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(0, 1, 0, 0),
            Padding = new Thickness(24, 12),
            Child = new Grid
            {
                ColumnDefinitions = ColumnDefinitions.Parse("*,Auto"),
                Children = { statusText, revisionText },
            },
        };
        Grid.SetColumn(revisionText, 1);
        Grid.SetRow(bar, 1);
        return bar;
    }

    private void OnActionInvoked(string nodeId)
    {
        statusText.Text = $"Action node emitted: {nodeId}";
        statusText.Foreground = LeserpentTheme.Accent;
    }

    private static void RequireExpectedDocument(UiDocument actual, UiDocument expected)
    {
        if (!JsonNode.DeepEquals(
            JsonSerializer.SerializeToNode(actual, RendererJsonContext.Default.UiDocument),
            JsonSerializer.SerializeToNode(expected, RendererJsonContext.Default.UiDocument)))
        {
            throw new InvalidDataException("Avalonia patch result does not match the fixture");
        }
    }

    private static bool RemoveNode(UiNode parent, string nodeId)
    {
        var index = parent.Children.FindIndex(child => child.Id == nodeId);
        if (index >= 0)
        {
            parent.Children.RemoveAt(index);
            return true;
        }
        return parent.Children.Any(child => RemoveNode(child, nodeId));
    }

    private static UiNode? FindNode(UiNode node, string nodeId)
    {
        if (node.Id == nodeId)
        {
            return node;
        }
        return node.Children.Select(FindChild).FirstOrDefault(found => found is not null);

        UiNode? FindChild(UiNode child) => FindNode(child, nodeId);
    }

    private static UiNode CloneShallow(UiNode node) => new()
    {
        Id = node.Id,
        Kind = node.Kind,
        RuntimeId = node.RuntimeId,
        DebuggerSessionId = node.DebuggerSessionId,
        Text = node.Text is null ? null : new LocalizedText
        {
            Key = node.Text.Key,
            Fallback = node.Text.Fallback,
        },
        Accessibility = new Accessibility
        {
            Label = node.Accessibility.Label is null ? null : new LocalizedText
            {
                Key = node.Accessibility.Label.Key,
                Fallback = node.Accessibility.Label.Fallback,
            },
            Description = node.Accessibility.Description is null ? null : new LocalizedText
            {
                Key = node.Accessibility.Description.Key,
                Fallback = node.Accessibility.Description.Fallback,
            },
        },
        Action = node.Action is null ? null : new UiAction
        {
            Kind = node.Action.Kind,
            RuntimeId = node.Action.RuntimeId,
            SessionId = node.Action.SessionId,
        },
        Children = [],
    };
}
