using System.Text.Json;
using System.Text.Json.Nodes;

const int MaxPayloadBytes = 2 * 1024 * 1024;

if (args.Length != 1)
{
    Console.Error.WriteLine("usage: Leserpent.RendererConformance FIXTURE");
    return 2;
}

var payload = ReadBoundedFixture(args[0]);
var fixture = JsonSerializer.Deserialize(
    payload,
    RendererJsonContext.Default.RendererFixture)
    ?? throw new InvalidDataException("fixture is empty");
if (fixture.SchemaVersion != 1)
{
    throw new InvalidDataException("unsupported fixture schema");
}

var renderer = new SemanticRenderer();
renderer.Mount(fixture.Previous);
renderer.Apply(fixture.Patch);
var actual = JsonSerializer.SerializeToNode(
    renderer.Document,
    RendererJsonContext.Default.UiDocument);
var expected = JsonSerializer.SerializeToNode(
    fixture.Next,
    RendererJsonContext.Default.UiDocument);
if (!JsonNode.DeepEquals(actual, expected))
{
    throw new InvalidDataException("incremental render does not match the next document");
}

if (fixture.PresentationOperation is not { } operation)
{
    Console.WriteLine($"renderer conformance valid: revision={renderer.Document.Revision}");
    return 0;
}

var operationPayload = JsonSerializer.SerializeToUtf8Bytes(
    operation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedOperation = JsonSerializer.Deserialize(
    operationPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("presentation operation round trip failed");
var navigationOperation = fixture.NavigationOperation
    ?? throw new InvalidDataException("presentation fixture contains no navigation operation");
var navigationPayload = JsonSerializer.SerializeToUtf8Bytes(
    navigationOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedNavigationOperation = JsonSerializer.Deserialize(
    navigationPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("navigation operation round trip failed");
var navigationFirstOperation = fixture.NavigationFirstOperation
    ?? throw new InvalidDataException("presentation fixture contains no first navigation operation");
var navigationFirstPayload = JsonSerializer.SerializeToUtf8Bytes(
    navigationFirstOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedNavigationFirstOperation = JsonSerializer.Deserialize(
    navigationFirstPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("first navigation operation round trip failed");
var navigationLastOperation = fixture.NavigationLastOperation
    ?? throw new InvalidDataException("presentation fixture contains no last navigation operation");
var navigationLastPayload = JsonSerializer.SerializeToUtf8Bytes(
    navigationLastOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedNavigationLastOperation = JsonSerializer.Deserialize(
    navigationLastPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("last navigation operation round trip failed");
var scrollOperation = fixture.ScrollOperation
    ?? throw new InvalidDataException("presentation fixture contains no scroll operation");
var scrollPayload = JsonSerializer.SerializeToUtf8Bytes(
    scrollOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedScrollOperation = JsonSerializer.Deserialize(
    scrollPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("scroll operation round trip failed");
var assertOperation = fixture.AssertOperation
    ?? throw new InvalidDataException("presentation fixture contains no assert operation");
var assertPayload = JsonSerializer.SerializeToUtf8Bytes(
    assertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedAssertOperation = JsonSerializer.Deserialize(
    assertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("assert operation round trip failed");
var realizedAssertOperation = fixture.RealizedAssertOperation
    ?? throw new InvalidDataException("presentation fixture contains no realized assert operation");
var realizedAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    realizedAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedRealizedAssertOperation = JsonSerializer.Deserialize(
    realizedAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("realized assert operation round trip failed");
var realizedWaitOperation = fixture.RealizedWaitOperation
    ?? throw new InvalidDataException("presentation fixture contains no realized wait operation");
var realizedWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    realizedWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedRealizedWaitOperation = JsonSerializer.Deserialize(
    realizedWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("realized wait operation round trip failed");
var visibleWaitOperation = fixture.VisibleWaitOperation
    ?? throw new InvalidDataException("presentation fixture contains no visible wait operation");
var visibleWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    visibleWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedVisibleWaitOperation = JsonSerializer.Deserialize(
    visibleWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("visible wait operation round trip failed");
var enabledWaitOperation = fixture.EnabledWaitOperation
    ?? throw new InvalidDataException("presentation fixture contains no enabled wait operation");
var enabledWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    enabledWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedEnabledWaitOperation = JsonSerializer.Deserialize(
    enabledWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("enabled wait operation round trip failed");
var focusedWaitOperation = fixture.FocusedWaitOperation
    ?? throw new InvalidDataException("presentation fixture contains no focused wait operation");
var focusedWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    focusedWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedFocusedWaitOperation = JsonSerializer.Deserialize(
    focusedWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("focused wait operation round trip failed");
var focusedAssertOperation = fixture.FocusedAssertOperation
    ?? throw new InvalidDataException("presentation fixture contains no focused assert operation");
var focusedAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    focusedAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedFocusedAssertOperation = JsonSerializer.Deserialize(
    focusedAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("focused assert operation round trip failed");
var enabledAssertOperation = fixture.EnabledAssertOperation
    ?? throw new InvalidDataException("presentation fixture contains no enabled assert operation");
var enabledAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    enabledAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedEnabledAssertOperation = JsonSerializer.Deserialize(
    enabledAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("enabled assert operation round trip failed");
var selectionAssertOperation = fixture.SelectionAssertOperation
    ?? throw new InvalidDataException("presentation fixture contains no selection assert operation");
var selectionAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    selectionAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedSelectionAssertOperation = JsonSerializer.Deserialize(
    selectionAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("selection assert operation round trip failed");
var selectionWaitOperation = fixture.SelectionWaitOperation
    ?? throw new InvalidDataException("presentation fixture contains no selection wait operation");
var selectionWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    selectionWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedSelectionWaitOperation = JsonSerializer.Deserialize(
    selectionWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("selection wait operation round trip failed");
var textAssertOperation = fixture.TextAssertOperation
    ?? throw new InvalidDataException("presentation fixture contains no text assert operation");
var textAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    textAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedTextAssertOperation = JsonSerializer.Deserialize(
    textAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("text assert operation round trip failed");
var automationIdAssertOperation = fixture.AutomationIdAssertOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no automation id assert operation");
var automationIdAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    automationIdAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedAutomationIdAssertOperation = JsonSerializer.Deserialize(
    automationIdAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("automation id assert operation round trip failed");
var nodeKindAssertOperation = fixture.NodeKindAssertOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no node kind assert operation");
var nodeKindAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    nodeKindAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedNodeKindAssertOperation = JsonSerializer.Deserialize(
    nodeKindAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("node kind assert operation round trip failed");
var actionKindAssertOperation = fixture.ActionKindAssertOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no action kind assert operation");
var actionKindAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    actionKindAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedActionKindAssertOperation = JsonSerializer.Deserialize(
    actionKindAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("action kind assert operation round trip failed");
var accessibleNameAssertOperation = fixture.AccessibleNameAssertOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no accessible name assert operation");
var accessibleNameAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    accessibleNameAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedAccessibleNameAssertOperation = JsonSerializer.Deserialize(
    accessibleNameAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("accessible name assert operation round trip failed");
var accessibleDescriptionAssertOperation = fixture.AccessibleDescriptionAssertOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no accessible description assert operation");
var accessibleDescriptionAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    accessibleDescriptionAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedAccessibleDescriptionAssertOperation = JsonSerializer.Deserialize(
    accessibleDescriptionAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("accessible description assert operation round trip failed");
if (renderer.ValidatePresentationOperation(decodedOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedNavigationOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedNavigationFirstOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedNavigationLastOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedScrollOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedAssertOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedRealizedAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedRealizedWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedVisibleWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedEnabledWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedFocusedWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedFocusedAssertOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedEnabledAssertOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedSelectionAssertOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedSelectionWaitOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedTextAssertOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedAutomationIdAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedNodeKindAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedActionKindAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedAccessibleNameAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedAccessibleDescriptionAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.Focus,
        NodeId = "missing-presentation-target",
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.NavigateFocus,
        NodeId = "missing-presentation-target",
        Direction = UiFocusNavigationDirection.Next,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.NavigateFocus,
        NodeId = renderer.Document.Root.Id,
        Direction = UiFocusNavigationDirection.Previous,
    }) != UiPresentationValidation.UnfocusableTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.NavigateFocus,
        NodeId = decodedNavigationOperation.NodeId,
    }) != UiPresentationValidation.InvalidNavigationDirection
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.Focus,
        NodeId = decodedOperation.NodeId,
        Direction = UiFocusNavigationDirection.Next,
    }) != UiPresentationValidation.InvalidNavigationDirection
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.ScrollIntoView,
        NodeId = "missing-presentation-target",
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertVisible,
        NodeId = "missing-presentation-target",
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertRealized,
        NodeId = "missing-presentation-target",
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitRealized,
        NodeId = "missing-presentation-target",
        TimeoutMs = SemanticRenderer.WaitRealizedTimeoutMs,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitRealized,
        NodeId = decodedRealizedWaitOperation.NodeId,
        TimeoutMs = SemanticRenderer.WaitRealizedTimeoutMs + 1,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitVisible,
        NodeId = "missing-presentation-target",
        TimeoutMs = SemanticRenderer.WaitVisibleTimeoutMs,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitVisible,
        NodeId = decodedVisibleWaitOperation.NodeId,
        TimeoutMs = SemanticRenderer.WaitVisibleTimeoutMs + 1,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitEnabled,
        NodeId = "missing-presentation-target",
        TimeoutMs = SemanticRenderer.WaitEnabledTimeoutMs,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitEnabled,
        NodeId = renderer.Document.Root.Id,
        TimeoutMs = SemanticRenderer.WaitEnabledTimeoutMs,
    }) != UiPresentationValidation.UnfocusableTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitEnabled,
        NodeId = decodedEnabledWaitOperation.NodeId,
        TimeoutMs = SemanticRenderer.WaitEnabledTimeoutMs + 1,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitFocused,
        NodeId = "missing-presentation-target",
        TimeoutMs = SemanticRenderer.WaitFocusedTimeoutMs,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitFocused,
        NodeId = renderer.Document.Root.Id,
        TimeoutMs = SemanticRenderer.WaitFocusedTimeoutMs,
    }) != UiPresentationValidation.UnfocusableTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitFocused,
        NodeId = decodedFocusedWaitOperation.NodeId,
        TimeoutMs = SemanticRenderer.WaitFocusedTimeoutMs + 1,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertRealized,
        NodeId = decodedRealizedAssertOperation.NodeId,
        TimeoutMs = SemanticRenderer.WaitRealizedTimeoutMs,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFocused,
        NodeId = "missing-presentation-target",
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertEnabled,
        NodeId = "missing-presentation-target",
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.Focus,
        NodeId = renderer.Document.Root.Id,
    }) != UiPresentationValidation.UnfocusableTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertEnabled,
        NodeId = renderer.Document.Root.Id,
    }) != UiPresentationValidation.UnfocusableTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertSelection,
        NodeId = "missing-presentation-target",
        State = UiSelectionState.Selected,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertSelection,
        NodeId = decodedSelectionAssertOperation.NodeId,
    }) != UiPresentationValidation.InvalidSelectionState
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertEnabled,
        NodeId = decodedEnabledAssertOperation.NodeId,
        State = UiSelectionState.Selected,
    }) != UiPresentationValidation.InvalidSelectionState
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertSelection,
        NodeId = "fleet-title",
        State = UiSelectionState.Selected,
    }) != UiPresentationValidation.SelectionlessTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitSelection,
        NodeId = decodedSelectionWaitOperation.NodeId,
        State = UiSelectionState.Unselected,
        TimeoutMs = SemanticRenderer.WaitSelectionTimeoutMs + 1,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertText,
        NodeId = renderer.Document.Root.Id,
        Expected = "Runtime fleet",
    }) != UiPresentationValidation.TextlessTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertText,
        NodeId = decodedTextAssertOperation.NodeId,
        Expected = "bad\ntext",
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertAutomationId,
        NodeId = decodedAutomationIdAssertOperation.NodeId,
        Expected = "bad/node",
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertNodeKind,
        NodeId = decodedNodeKindAssertOperation.NodeId,
    }) != UiPresentationValidation.InvalidExpectedKind
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertText,
        NodeId = decodedTextAssertOperation.NodeId,
        Expected = "Runtime fleet",
        ExpectedKind = UiNodeKind.Heading,
    }) != UiPresentationValidation.InvalidExpectedKind
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertActionKind,
        NodeId = decodedActionKindAssertOperation.NodeId,
    }) != UiPresentationValidation.InvalidExpectedActionKind
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertActionKind,
        NodeId = decodedActionKindAssertOperation.NodeId,
        ExpectedKind = UiNodeKind.Action,
    }) != UiPresentationValidation.InvalidExpectedKind
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertText,
        NodeId = decodedTextAssertOperation.NodeId,
        Expected = "Runtime fleet",
        ExpectedActionKind = ActionKind.RuntimeRefresh,
    }) != UiPresentationValidation.InvalidExpectedActionKind
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertActionKind,
        NodeId = renderer.Document.Root.Id,
        ExpectedActionKind = ActionKind.RuntimeRefresh,
    }) != UiPresentationValidation.UnfocusableTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertAccessibleName,
        NodeId = decodedAccessibleNameAssertOperation.NodeId,
        Expected = "bad\nname",
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertAccessibleDescription,
        NodeId = decodedAccessibleDescriptionAssertOperation.NodeId,
        Expected = "bad\ndescription",
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertAccessibleDescription,
        NodeId = renderer.Document.Root.Id,
        Expected = "description",
    }) != UiPresentationValidation.DescriptionlessTarget)
{
    throw new InvalidDataException("presentation operation validation diverged");
}

Console.WriteLine(
    $"renderer conformance valid: revision={renderer.Document.Revision}, presentation_focus=true, presentation_navigate_focus=true, presentation_navigate_focus_first_last=true, presentation_scroll_into_view=true, presentation_assert_visible=true, presentation_assert_realized=true, presentation_wait_realized=true, presentation_wait_visible=true, presentation_wait_enabled=true, presentation_wait_focused=true, presentation_assert_focused=true, presentation_assert_enabled=true, presentation_assert_selection=true, presentation_wait_selection=true, presentation_assert_text=true, presentation_assert_automation_id=true, presentation_assert_node_kind=true, presentation_assert_action_kind=true, presentation_assert_accessible_name=true, presentation_assert_accessible_description=true, strict_codec=true");
return 0;

static byte[] ReadBoundedFixture(string path)
{
    using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read);
    if (stream.Length > MaxPayloadBytes)
    {
        throw new InvalidDataException("fixture exceeds the UI IR payload limit");
    }

    var payload = new byte[checked((int)stream.Length)];
    stream.ReadExactly(payload);
    if (stream.ReadByte() != -1)
    {
        throw new InvalidDataException("fixture changed while being read");
    }
    return payload;
}
