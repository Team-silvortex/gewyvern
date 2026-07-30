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
var hiddenAssertOperation = fixture.HiddenAssertOperation
    ?? throw new InvalidDataException("presentation fixture contains no hidden assert operation");
var hiddenAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    hiddenAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedHiddenAssertOperation = JsonSerializer.Deserialize(
    hiddenAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("hidden assert operation round trip failed");
var hiddenWaitOperation = fixture.HiddenWaitOperation
    ?? throw new InvalidDataException("presentation fixture contains no hidden wait operation");
var hiddenWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    hiddenWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedHiddenWaitOperation = JsonSerializer.Deserialize(
    hiddenWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("hidden wait operation round trip failed");
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
var disabledWaitOperation = fixture.DisabledWaitOperation
    ?? throw new InvalidDataException("presentation fixture contains no disabled wait operation");
var disabledWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    disabledWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedDisabledWaitOperation = JsonSerializer.Deserialize(
    disabledWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("disabled wait operation round trip failed");
var windowOpenAssertOperation = fixture.WindowOpenAssertOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no window-open assert operation");
var windowOpenAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    windowOpenAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedWindowOpenAssertOperation = JsonSerializer.Deserialize(
    windowOpenAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("window-open assert operation round trip failed");
var windowOpenWaitOperation = fixture.WindowOpenWaitOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no window-open wait operation");
var windowOpenWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    windowOpenWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedWindowOpenWaitOperation = JsonSerializer.Deserialize(
    windowOpenWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("window-open wait operation round trip failed");
var windowClosedAssertOperation = fixture.WindowClosedAssertOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no window-closed assert operation");
var windowClosedAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    windowClosedAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedWindowClosedAssertOperation = JsonSerializer.Deserialize(
    windowClosedAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("window-closed assert operation round trip failed");
var windowClosedWaitOperation = fixture.WindowClosedWaitOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no window-closed wait operation");
var windowClosedWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    windowClosedWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedWindowClosedWaitOperation = JsonSerializer.Deserialize(
    windowClosedWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("window-closed wait operation round trip failed");
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
var unfocusedWaitOperation = fixture.UnfocusedWaitOperation
    ?? throw new InvalidDataException("presentation fixture contains no unfocused wait operation");
var unfocusedWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    unfocusedWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedUnfocusedWaitOperation = JsonSerializer.Deserialize(
    unfocusedWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("unfocused wait operation round trip failed");
var unfocusedAssertOperation = fixture.UnfocusedAssertOperation
    ?? throw new InvalidDataException("presentation fixture contains no unfocused assert operation");
var unfocusedAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    unfocusedAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedUnfocusedAssertOperation = JsonSerializer.Deserialize(
    unfocusedAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("unfocused assert operation round trip failed");
var enabledAssertOperation = fixture.EnabledAssertOperation
    ?? throw new InvalidDataException("presentation fixture contains no enabled assert operation");
var enabledAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    enabledAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedEnabledAssertOperation = JsonSerializer.Deserialize(
    enabledAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("enabled assert operation round trip failed");
var disabledAssertOperation = fixture.DisabledAssertOperation
    ?? throw new InvalidDataException("presentation fixture contains no disabled assert operation");
var disabledAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    disabledAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedDisabledAssertOperation = JsonSerializer.Deserialize(
    disabledAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("disabled assert operation round trip failed");
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
var textWaitOperation = fixture.TextWaitOperation
    ?? throw new InvalidDataException("presentation fixture contains no text wait operation");
var textWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    textWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedTextWaitOperation = JsonSerializer.Deserialize(
    textWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("text wait operation round trip failed");
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
var nodeKindWaitOperation = fixture.NodeKindWaitOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no node kind wait operation");
var nodeKindWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    nodeKindWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedNodeKindWaitOperation = JsonSerializer.Deserialize(
    nodeKindWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("node kind wait operation round trip failed");
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
var actionKindWaitOperation = fixture.ActionKindWaitOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no action kind wait operation");
var actionKindWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    actionKindWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedActionKindWaitOperation = JsonSerializer.Deserialize(
    actionKindWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("action kind wait operation round trip failed");
var actionLabelAssertOperation = fixture.ActionLabelAssertOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no action label assert operation");
var actionLabelAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    actionLabelAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedActionLabelAssertOperation = JsonSerializer.Deserialize(
    actionLabelAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("action label assert operation round trip failed");
var actionLabelWaitOperation = fixture.ActionLabelWaitOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no action label wait operation");
var actionLabelWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    actionLabelWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedActionLabelWaitOperation = JsonSerializer.Deserialize(
    actionLabelWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("action label wait operation round trip failed");
var actionAvailableAssertOperation = fixture.ActionAvailableAssertOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no action available assert operation");
var actionAvailableAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    actionAvailableAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedActionAvailableAssertOperation = JsonSerializer.Deserialize(
    actionAvailableAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException(
        "action available assert operation round trip failed");
var actionAvailableWaitOperation = fixture.ActionAvailableWaitOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no action available wait operation");
var actionAvailableWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    actionAvailableWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedActionAvailableWaitOperation = JsonSerializer.Deserialize(
    actionAvailableWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("action available wait operation round trip failed");
var actionUnavailableReasonAssertOperation = fixture.ActionUnavailableReasonAssertOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no action unavailable reason assert operation");
var actionUnavailableReasonAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    actionUnavailableReasonAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedActionUnavailableReasonAssertOperation = JsonSerializer.Deserialize(
    actionUnavailableReasonAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException(
        "action unavailable reason assert operation round trip failed");
var actionUnavailableReasonWaitOperation = fixture.ActionUnavailableReasonWaitOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no action unavailable reason wait operation");
var actionUnavailableReasonWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    actionUnavailableReasonWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedActionUnavailableReasonWaitOperation = JsonSerializer.Deserialize(
    actionUnavailableReasonWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException(
        "action unavailable reason wait operation round trip failed");
var formFieldAssertOperation = fixture.FormFieldAssertOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no form field assert operation");
var formFieldAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    formFieldAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedFormFieldAssertOperation = JsonSerializer.Deserialize(
    formFieldAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("form field assert operation round trip failed");
var decodedFormField = decodedFormFieldAssertOperation.Field
    ?? throw new InvalidDataException("form field assert operation contains no field");
var decodedFormFieldExpected = decodedFormFieldAssertOperation.Expected
    ?? throw new InvalidDataException("form field assert operation contains no expected text");
var formFieldInputKindAssertOperation = fixture.FormFieldInputKindAssertOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no form field input kind assert operation");
var formFieldInputKindAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    formFieldInputKindAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedFormFieldInputKindAssertOperation = JsonSerializer.Deserialize(
    formFieldInputKindAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException(
        "form field input kind assert operation round trip failed");
var decodedFormFieldInputKind = decodedFormFieldInputKindAssertOperation.Field
    ?? throw new InvalidDataException(
        "form field input kind assert operation contains no field");
var decodedFormFieldInputKindExpected = decodedFormFieldInputKindAssertOperation.InputKind
    ?? throw new InvalidDataException(
        "form field input kind assert operation contains no expected input kind");
var formFieldRequiredAssertOperation = fixture.FormFieldRequiredAssertOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no form field required assert operation");
var formFieldRequiredAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    formFieldRequiredAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedFormFieldRequiredAssertOperation = JsonSerializer.Deserialize(
    formFieldRequiredAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException(
        "form field required assert operation round trip failed");
var decodedFormFieldRequired = decodedFormFieldRequiredAssertOperation.Field
    ?? throw new InvalidDataException(
        "form field required assert operation contains no field");
var decodedFormFieldRequiredExpected = decodedFormFieldRequiredAssertOperation.Required
    ?? throw new InvalidDataException(
        "form field required assert operation contains no expected required state");
var formFieldMaxLengthAssertOperation = fixture.FormFieldMaxLengthAssertOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no form field max length assert operation");
var formFieldMaxLengthAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    formFieldMaxLengthAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedFormFieldMaxLengthAssertOperation = JsonSerializer.Deserialize(
    formFieldMaxLengthAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException(
        "form field max length assert operation round trip failed");
var decodedFormFieldMaxLength = decodedFormFieldMaxLengthAssertOperation.Field
    ?? throw new InvalidDataException(
        "form field max length assert operation contains no field");
var decodedFormFieldMaxLengthExpected = decodedFormFieldMaxLengthAssertOperation.MaxLength
    ?? throw new InvalidDataException(
        "form field max length assert operation contains no expected max length");
var formFieldPlaceholderAssertOperation = fixture.FormFieldPlaceholderAssertOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no form field placeholder assert operation");
var formFieldPlaceholderAssertPayload = JsonSerializer.SerializeToUtf8Bytes(
    formFieldPlaceholderAssertOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedFormFieldPlaceholderAssertOperation = JsonSerializer.Deserialize(
    formFieldPlaceholderAssertPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException(
        "form field placeholder assert operation round trip failed");
var decodedFormFieldPlaceholder = decodedFormFieldPlaceholderAssertOperation.Field
    ?? throw new InvalidDataException(
        "form field placeholder assert operation contains no field");
var decodedFormFieldPlaceholderExpected = decodedFormFieldPlaceholderAssertOperation.Expected
    ?? throw new InvalidDataException(
        "form field placeholder assert operation contains no expected placeholder");
var formFieldPlaceholderWaitOperation = fixture.FormFieldPlaceholderWaitOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no form field placeholder wait operation");
var formFieldPlaceholderWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    formFieldPlaceholderWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedFormFieldPlaceholderWaitOperation = JsonSerializer.Deserialize(
    formFieldPlaceholderWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException(
        "form field placeholder wait operation round trip failed");
var decodedFormFieldPlaceholderWait = decodedFormFieldPlaceholderWaitOperation.Field
    ?? throw new InvalidDataException(
        "form field placeholder wait operation contains no field");
var decodedFormFieldPlaceholderWaitExpected = decodedFormFieldPlaceholderWaitOperation.Expected
    ?? throw new InvalidDataException(
        "form field placeholder wait operation contains no expected placeholder");
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
var accessibleNameWaitOperation = fixture.AccessibleNameWaitOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no accessible name wait operation");
var accessibleNameWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    accessibleNameWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedAccessibleNameWaitOperation = JsonSerializer.Deserialize(
    accessibleNameWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("accessible name wait operation round trip failed");
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
var accessibleDescriptionWaitOperation = fixture.AccessibleDescriptionWaitOperation
    ?? throw new InvalidDataException(
        "presentation fixture contains no accessible description wait operation");
var accessibleDescriptionWaitPayload = JsonSerializer.SerializeToUtf8Bytes(
    accessibleDescriptionWaitOperation,
    RendererJsonContext.Default.UiPresentationOperation);
var decodedAccessibleDescriptionWaitOperation = JsonSerializer.Deserialize(
    accessibleDescriptionWaitPayload,
    RendererJsonContext.Default.UiPresentationOperation)
    ?? throw new InvalidDataException("accessible description wait operation round trip failed");
if (renderer.ValidatePresentationOperation(decodedOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedNavigationOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedNavigationFirstOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedNavigationLastOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedScrollOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedAssertOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedHiddenAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedHiddenWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedRealizedAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedRealizedWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedVisibleWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedEnabledWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedDisabledWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedWindowOpenAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedWindowOpenWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedWindowClosedAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedWindowClosedWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedFocusedWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedFocusedAssertOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedUnfocusedWaitOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedUnfocusedAssertOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedEnabledAssertOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedDisabledAssertOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedSelectionAssertOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedSelectionWaitOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedTextAssertOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedTextWaitOperation) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedAutomationIdAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedNodeKindAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedNodeKindWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedActionKindAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedActionKindWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedActionLabelAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedActionLabelWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedActionAvailableAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedActionAvailableWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedActionUnavailableReasonAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertActionUnavailableReason,
        NodeId = decodedActionUnavailableReasonAssertOperation.NodeId,
    }) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedActionUnavailableReasonWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitActionUnavailableReason,
        NodeId = decodedActionUnavailableReasonWaitOperation.NodeId,
        TimeoutMs = SemanticRenderer.WaitActionUnavailableReasonTimeoutMs,
    }) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedFormFieldAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedFormFieldInputKindAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedFormFieldRequiredAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedFormFieldMaxLengthAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedFormFieldPlaceholderAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldPlaceholder,
        NodeId = decodedFormFieldPlaceholderAssertOperation.NodeId,
        Field = decodedFormFieldPlaceholder,
    }) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedFormFieldPlaceholderWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitFormFieldPlaceholder,
        NodeId = decodedFormFieldPlaceholderWaitOperation.NodeId,
        Field = decodedFormFieldPlaceholderWait,
        TimeoutMs = SemanticRenderer.WaitFormFieldPlaceholderTimeoutMs,
    }) != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedAccessibleNameAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedAccessibleNameWaitOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedAccessibleDescriptionAssertOperation)
        != UiPresentationValidation.Valid
    || renderer.ValidatePresentationOperation(decodedAccessibleDescriptionWaitOperation)
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
        Kind = UiPresentationOperationKind.AssertHidden,
        NodeId = "missing-presentation-target",
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitHidden,
        NodeId = "missing-presentation-target",
        TimeoutMs = SemanticRenderer.WaitVisibleTimeoutMs,
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
        Kind = UiPresentationOperationKind.WaitHidden,
        NodeId = decodedHiddenWaitOperation.NodeId,
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
        Kind = UiPresentationOperationKind.WaitDisabled,
        NodeId = "missing-presentation-target",
        TimeoutMs = SemanticRenderer.WaitEnabledTimeoutMs,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitDisabled,
        NodeId = renderer.Document.Root.Id,
        TimeoutMs = SemanticRenderer.WaitEnabledTimeoutMs,
    }) != UiPresentationValidation.UnfocusableTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitDisabled,
        NodeId = decodedDisabledWaitOperation.NodeId,
        TimeoutMs = SemanticRenderer.WaitEnabledTimeoutMs + 1,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertWindowOpen,
        NodeId = "missing-presentation-target",
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertWindowOpen,
        NodeId = decodedWindowOpenAssertOperation.NodeId,
        TimeoutMs = 1,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitWindowOpen,
        NodeId = "missing-presentation-target",
        TimeoutMs = SemanticRenderer.WaitWindowOpenTimeoutMs,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitWindowOpen,
        NodeId = decodedWindowOpenWaitOperation.NodeId,
        TimeoutMs = SemanticRenderer.WaitWindowOpenTimeoutMs + 1,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertWindowClosed,
        NodeId = "missing-presentation-target",
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertWindowClosed,
        NodeId = decodedWindowClosedAssertOperation.NodeId,
        TimeoutMs = 1,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitWindowClosed,
        NodeId = "missing-presentation-target",
        TimeoutMs = SemanticRenderer.WaitWindowClosedTimeoutMs,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitWindowClosed,
        NodeId = decodedWindowClosedWaitOperation.NodeId,
        TimeoutMs = SemanticRenderer.WaitWindowClosedTimeoutMs + 1,
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
        Kind = UiPresentationOperationKind.WaitUnfocused,
        NodeId = "missing-presentation-target",
        TimeoutMs = SemanticRenderer.WaitUnfocusedTimeoutMs,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitUnfocused,
        NodeId = renderer.Document.Root.Id,
        TimeoutMs = SemanticRenderer.WaitUnfocusedTimeoutMs,
    }) != UiPresentationValidation.UnfocusableTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitUnfocused,
        NodeId = decodedUnfocusedWaitOperation.NodeId,
        TimeoutMs = SemanticRenderer.WaitUnfocusedTimeoutMs + 1,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertRealized,
        NodeId = decodedRealizedAssertOperation.NodeId,
        TimeoutMs = SemanticRenderer.WaitRealizedTimeoutMs,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertHidden,
        NodeId = decodedHiddenAssertOperation.NodeId,
        TimeoutMs = SemanticRenderer.WaitVisibleTimeoutMs,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFocused,
        NodeId = "missing-presentation-target",
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertUnfocused,
        NodeId = "missing-presentation-target",
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertUnfocused,
        NodeId = renderer.Document.Root.Id,
    }) != UiPresentationValidation.UnfocusableTarget
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
        Kind = UiPresentationOperationKind.AssertDisabled,
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
        Kind = UiPresentationOperationKind.WaitText,
        NodeId = renderer.Document.Root.Id,
        Expected = "Runtime fleet",
        TimeoutMs = SemanticRenderer.WaitTextTimeoutMs,
    }) != UiPresentationValidation.TextlessTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitText,
        NodeId = decodedTextWaitOperation.NodeId,
        Expected = "bad\ntext",
        TimeoutMs = SemanticRenderer.WaitTextTimeoutMs,
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitText,
        NodeId = decodedTextWaitOperation.NodeId,
        Expected = decodedTextWaitOperation.Expected,
        TimeoutMs = SemanticRenderer.WaitTextTimeoutMs + 1,
    }) != UiPresentationValidation.InvalidTimeout
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
        Kind = UiPresentationOperationKind.WaitNodeKind,
        NodeId = decodedNodeKindWaitOperation.NodeId,
        TimeoutMs = SemanticRenderer.WaitNodeKindTimeoutMs,
    }) != UiPresentationValidation.InvalidExpectedKind
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitNodeKind,
        NodeId = decodedNodeKindWaitOperation.NodeId,
        ExpectedKind = decodedNodeKindWaitOperation.ExpectedKind,
        TimeoutMs = SemanticRenderer.WaitNodeKindTimeoutMs + 1,
    }) != UiPresentationValidation.InvalidTimeout
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
        Kind = UiPresentationOperationKind.WaitActionKind,
        NodeId = decodedActionKindWaitOperation.NodeId,
        TimeoutMs = SemanticRenderer.WaitActionKindTimeoutMs,
    }) != UiPresentationValidation.InvalidExpectedActionKind
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertActionKind,
        NodeId = decodedActionKindAssertOperation.NodeId,
        ExpectedKind = UiNodeKind.Action,
    }) != UiPresentationValidation.InvalidExpectedKind
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitActionKind,
        NodeId = decodedActionKindWaitOperation.NodeId,
        ExpectedActionKind = decodedActionKindWaitOperation.ExpectedActionKind,
        TimeoutMs = SemanticRenderer.WaitActionKindTimeoutMs + 1,
    }) != UiPresentationValidation.InvalidTimeout
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
        Kind = UiPresentationOperationKind.WaitActionKind,
        NodeId = renderer.Document.Root.Id,
        ExpectedActionKind = ActionKind.RuntimeRefresh,
        TimeoutMs = SemanticRenderer.WaitActionKindTimeoutMs,
    }) != UiPresentationValidation.UnfocusableTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertActionLabel,
        NodeId = "missing-presentation-target",
        Expected = decodedActionLabelAssertOperation.Expected,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertActionLabel,
        NodeId = renderer.Document.Root.Id,
        Expected = decodedActionLabelAssertOperation.Expected,
    }) != UiPresentationValidation.UnfocusableTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertActionLabel,
        NodeId = decodedActionLabelAssertOperation.NodeId,
        Expected = "bad\nlabel",
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertActionLabel,
        NodeId = decodedActionLabelAssertOperation.NodeId,
        Expected = decodedActionLabelAssertOperation.Expected,
        ExpectedActionKind = ActionKind.RuntimeRefresh,
    }) != UiPresentationValidation.InvalidExpectedActionKind
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitActionLabel,
        NodeId = "missing-presentation-target",
        Expected = decodedActionLabelWaitOperation.Expected,
        TimeoutMs = SemanticRenderer.WaitActionLabelTimeoutMs,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitActionLabel,
        NodeId = renderer.Document.Root.Id,
        Expected = decodedActionLabelWaitOperation.Expected,
        TimeoutMs = SemanticRenderer.WaitActionLabelTimeoutMs,
    }) != UiPresentationValidation.UnfocusableTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitActionLabel,
        NodeId = decodedActionLabelWaitOperation.NodeId,
        Expected = "bad\nlabel",
        TimeoutMs = SemanticRenderer.WaitActionLabelTimeoutMs,
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitActionLabel,
        NodeId = decodedActionLabelWaitOperation.NodeId,
        Expected = decodedActionLabelWaitOperation.Expected,
        TimeoutMs = SemanticRenderer.WaitActionLabelTimeoutMs + 1,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitActionLabel,
        NodeId = decodedActionLabelWaitOperation.NodeId,
        Expected = decodedActionLabelWaitOperation.Expected,
        ExpectedActionKind = ActionKind.RuntimeRefresh,
        TimeoutMs = SemanticRenderer.WaitActionLabelTimeoutMs,
    }) != UiPresentationValidation.InvalidExpectedActionKind
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertActionAvailable,
        NodeId = "missing-presentation-target",
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertActionAvailable,
        NodeId = renderer.Document.Root.Id,
    }) != UiPresentationValidation.UnfocusableTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertActionAvailable,
        NodeId = decodedActionAvailableAssertOperation.NodeId,
        Expected = "available",
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitActionAvailable,
        NodeId = "missing-presentation-target",
        TimeoutMs = SemanticRenderer.WaitActionAvailableTimeoutMs,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitActionAvailable,
        NodeId = renderer.Document.Root.Id,
        TimeoutMs = SemanticRenderer.WaitActionAvailableTimeoutMs,
    }) != UiPresentationValidation.UnfocusableTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitActionAvailable,
        NodeId = decodedActionAvailableWaitOperation.NodeId,
        TimeoutMs = SemanticRenderer.WaitActionAvailableTimeoutMs + 1,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitActionAvailable,
        NodeId = decodedActionAvailableWaitOperation.NodeId,
        Expected = "available",
        TimeoutMs = SemanticRenderer.WaitActionAvailableTimeoutMs,
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertActionUnavailableReason,
        NodeId = "missing-presentation-target",
        Expected = decodedActionUnavailableReasonAssertOperation.Expected,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertActionUnavailableReason,
        NodeId = renderer.Document.Root.Id,
        Expected = decodedActionUnavailableReasonAssertOperation.Expected,
    }) != UiPresentationValidation.UnfocusableTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertActionUnavailableReason,
        NodeId = decodedActionUnavailableReasonAssertOperation.NodeId,
        Expected = "bad\nreason",
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertActionUnavailableReason,
        NodeId = decodedActionUnavailableReasonAssertOperation.NodeId,
        ExpectedActionKind = ActionKind.RuntimeRefresh,
    }) != UiPresentationValidation.InvalidExpectedActionKind
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitActionUnavailableReason,
        NodeId = "missing-presentation-target",
        Expected = decodedActionUnavailableReasonWaitOperation.Expected,
        TimeoutMs = SemanticRenderer.WaitActionUnavailableReasonTimeoutMs,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitActionUnavailableReason,
        NodeId = renderer.Document.Root.Id,
        Expected = decodedActionUnavailableReasonWaitOperation.Expected,
        TimeoutMs = SemanticRenderer.WaitActionUnavailableReasonTimeoutMs,
    }) != UiPresentationValidation.UnfocusableTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitActionUnavailableReason,
        NodeId = decodedActionUnavailableReasonWaitOperation.NodeId,
        Expected = "bad\nreason",
        TimeoutMs = SemanticRenderer.WaitActionUnavailableReasonTimeoutMs,
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitActionUnavailableReason,
        NodeId = decodedActionUnavailableReasonWaitOperation.NodeId,
        Expected = decodedActionUnavailableReasonWaitOperation.Expected,
        TimeoutMs = SemanticRenderer.WaitActionUnavailableReasonTimeoutMs + 1,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitActionUnavailableReason,
        NodeId = decodedActionUnavailableReasonWaitOperation.NodeId,
        ExpectedActionKind = ActionKind.RuntimeRefresh,
        TimeoutMs = SemanticRenderer.WaitActionUnavailableReasonTimeoutMs,
    }) != UiPresentationValidation.InvalidExpectedActionKind
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormField,
        NodeId = "missing-presentation-target",
        Field = decodedFormField,
        Expected = decodedFormFieldExpected,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormField,
        NodeId = renderer.Document.Root.Id,
        Field = decodedFormField,
        Expected = decodedFormFieldExpected,
    }) != UiPresentationValidation.FormlessTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormField,
        NodeId = decodedFormFieldAssertOperation.NodeId,
        Field = "missing",
        Expected = decodedFormFieldExpected,
    }) != UiPresentationValidation.UnknownFormField
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormField,
        NodeId = decodedFormFieldAssertOperation.NodeId,
        Field = "bad/field",
        Expected = decodedFormFieldExpected,
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormField,
        NodeId = decodedFormFieldAssertOperation.NodeId,
        Field = decodedFormField,
        Expected = "bad\nlabel",
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldInputKind,
        NodeId = "missing-presentation-target",
        Field = decodedFormFieldInputKind,
        InputKind = decodedFormFieldInputKindExpected,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldInputKind,
        NodeId = renderer.Document.Root.Id,
        Field = decodedFormFieldInputKind,
        InputKind = decodedFormFieldInputKindExpected,
    }) != UiPresentationValidation.FormlessTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldInputKind,
        NodeId = decodedFormFieldInputKindAssertOperation.NodeId,
        Field = "missing",
        InputKind = decodedFormFieldInputKindExpected,
    }) != UiPresentationValidation.UnknownFormField
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldInputKind,
        NodeId = decodedFormFieldInputKindAssertOperation.NodeId,
        Field = "bad/field",
        InputKind = decodedFormFieldInputKindExpected,
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldInputKind,
        NodeId = decodedFormFieldInputKindAssertOperation.NodeId,
        Field = decodedFormFieldInputKind,
    }) != UiPresentationValidation.InvalidExpectedInputKind
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertText,
        NodeId = decodedTextAssertOperation.NodeId,
        Expected = "Runtime fleet",
        InputKind = decodedFormFieldInputKindExpected,
    }) != UiPresentationValidation.InvalidExpectedInputKind
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldRequired,
        NodeId = "missing-presentation-target",
        Field = decodedFormFieldRequired,
        Required = decodedFormFieldRequiredExpected,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldRequired,
        NodeId = renderer.Document.Root.Id,
        Field = decodedFormFieldRequired,
        Required = decodedFormFieldRequiredExpected,
    }) != UiPresentationValidation.FormlessTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldRequired,
        NodeId = decodedFormFieldRequiredAssertOperation.NodeId,
        Field = "missing",
        Required = decodedFormFieldRequiredExpected,
    }) != UiPresentationValidation.UnknownFormField
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldRequired,
        NodeId = decodedFormFieldRequiredAssertOperation.NodeId,
        Field = "bad/field",
        Required = decodedFormFieldRequiredExpected,
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldRequired,
        NodeId = decodedFormFieldRequiredAssertOperation.NodeId,
        Field = decodedFormFieldRequired,
    }) != UiPresentationValidation.InvalidExpectedRequired
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertText,
        NodeId = decodedTextAssertOperation.NodeId,
        Expected = "Runtime fleet",
        Required = decodedFormFieldRequiredExpected,
    }) != UiPresentationValidation.InvalidExpectedRequired
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldMaxLength,
        NodeId = "missing-presentation-target",
        Field = decodedFormFieldMaxLength,
        MaxLength = decodedFormFieldMaxLengthExpected,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldMaxLength,
        NodeId = renderer.Document.Root.Id,
        Field = decodedFormFieldMaxLength,
        MaxLength = decodedFormFieldMaxLengthExpected,
    }) != UiPresentationValidation.FormlessTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldMaxLength,
        NodeId = decodedFormFieldMaxLengthAssertOperation.NodeId,
        Field = "missing",
        MaxLength = decodedFormFieldMaxLengthExpected,
    }) != UiPresentationValidation.UnknownFormField
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldMaxLength,
        NodeId = decodedFormFieldMaxLengthAssertOperation.NodeId,
        Field = "bad/field",
        MaxLength = decodedFormFieldMaxLengthExpected,
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldMaxLength,
        NodeId = decodedFormFieldMaxLengthAssertOperation.NodeId,
        Field = decodedFormFieldMaxLength,
    }) != UiPresentationValidation.InvalidExpectedMaxLength
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldMaxLength,
        NodeId = decodedFormFieldMaxLengthAssertOperation.NodeId,
        Field = decodedFormFieldMaxLength,
        MaxLength = 0,
    }) != UiPresentationValidation.InvalidExpectedMaxLength
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldMaxLength,
        NodeId = decodedFormFieldMaxLengthAssertOperation.NodeId,
        Field = decodedFormFieldMaxLength,
        MaxLength = 257,
    }) != UiPresentationValidation.InvalidExpectedMaxLength
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertText,
        NodeId = decodedTextAssertOperation.NodeId,
        Expected = "Runtime fleet",
        MaxLength = decodedFormFieldMaxLengthExpected,
    }) != UiPresentationValidation.InvalidExpectedMaxLength
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldPlaceholder,
        NodeId = "missing-presentation-target",
        Field = decodedFormFieldPlaceholder,
        Expected = decodedFormFieldPlaceholderExpected,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldPlaceholder,
        NodeId = renderer.Document.Root.Id,
        Field = decodedFormFieldPlaceholder,
        Expected = decodedFormFieldPlaceholderExpected,
    }) != UiPresentationValidation.FormlessTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldPlaceholder,
        NodeId = decodedFormFieldPlaceholderAssertOperation.NodeId,
        Field = "missing",
        Expected = decodedFormFieldPlaceholderExpected,
    }) != UiPresentationValidation.UnknownFormField
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldPlaceholder,
        NodeId = decodedFormFieldPlaceholderAssertOperation.NodeId,
        Field = "bad/field",
        Expected = decodedFormFieldPlaceholderExpected,
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertFormFieldPlaceholder,
        NodeId = decodedFormFieldPlaceholderAssertOperation.NodeId,
        Field = decodedFormFieldPlaceholder,
        Expected = "bad\nplaceholder",
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitFormFieldPlaceholder,
        NodeId = "missing-presentation-target",
        Field = decodedFormFieldPlaceholderWait,
        Expected = decodedFormFieldPlaceholderWaitExpected,
        TimeoutMs = SemanticRenderer.WaitFormFieldPlaceholderTimeoutMs,
    }) != UiPresentationValidation.UnknownTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitFormFieldPlaceholder,
        NodeId = renderer.Document.Root.Id,
        Field = decodedFormFieldPlaceholderWait,
        Expected = decodedFormFieldPlaceholderWaitExpected,
        TimeoutMs = SemanticRenderer.WaitFormFieldPlaceholderTimeoutMs,
    }) != UiPresentationValidation.FormlessTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitFormFieldPlaceholder,
        NodeId = decodedFormFieldPlaceholderWaitOperation.NodeId,
        Field = "missing",
        Expected = decodedFormFieldPlaceholderWaitExpected,
        TimeoutMs = SemanticRenderer.WaitFormFieldPlaceholderTimeoutMs,
    }) != UiPresentationValidation.UnknownFormField
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitFormFieldPlaceholder,
        NodeId = decodedFormFieldPlaceholderWaitOperation.NodeId,
        Field = "bad/field",
        Expected = decodedFormFieldPlaceholderWaitExpected,
        TimeoutMs = SemanticRenderer.WaitFormFieldPlaceholderTimeoutMs,
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitFormFieldPlaceholder,
        NodeId = decodedFormFieldPlaceholderWaitOperation.NodeId,
        Field = decodedFormFieldPlaceholderWait,
        Expected = "bad\nplaceholder",
        TimeoutMs = SemanticRenderer.WaitFormFieldPlaceholderTimeoutMs,
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitFormFieldPlaceholder,
        NodeId = decodedFormFieldPlaceholderWaitOperation.NodeId,
        Field = decodedFormFieldPlaceholderWait,
        Expected = decodedFormFieldPlaceholderWaitExpected,
        TimeoutMs = SemanticRenderer.WaitFormFieldPlaceholderTimeoutMs + 1,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertAccessibleName,
        NodeId = decodedAccessibleNameAssertOperation.NodeId,
        Expected = "bad\nname",
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitAccessibleName,
        NodeId = decodedAccessibleNameWaitOperation.NodeId,
        Expected = "bad\nname",
        TimeoutMs = SemanticRenderer.WaitAccessibleNameTimeoutMs,
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitAccessibleName,
        NodeId = decodedAccessibleNameWaitOperation.NodeId,
        Expected = decodedAccessibleNameWaitOperation.Expected,
        TimeoutMs = SemanticRenderer.WaitAccessibleNameTimeoutMs + 1,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertAccessibleDescription,
        NodeId = decodedAccessibleDescriptionAssertOperation.NodeId,
        Expected = "bad\ndescription",
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitAccessibleDescription,
        NodeId = decodedAccessibleDescriptionWaitOperation.NodeId,
        Expected = "bad\ndescription",
        TimeoutMs = SemanticRenderer.WaitAccessibleDescriptionTimeoutMs,
    }) != UiPresentationValidation.InvalidExpectedText
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitAccessibleDescription,
        NodeId = decodedAccessibleDescriptionWaitOperation.NodeId,
        Expected = decodedAccessibleDescriptionWaitOperation.Expected,
        TimeoutMs = SemanticRenderer.WaitAccessibleDescriptionTimeoutMs + 1,
    }) != UiPresentationValidation.InvalidTimeout
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.AssertAccessibleDescription,
        NodeId = renderer.Document.Root.Id,
        Expected = "description",
    }) != UiPresentationValidation.DescriptionlessTarget
    || renderer.ValidatePresentationOperation(new UiPresentationOperation
    {
        Kind = UiPresentationOperationKind.WaitAccessibleDescription,
        NodeId = renderer.Document.Root.Id,
        Expected = "description",
        TimeoutMs = SemanticRenderer.WaitAccessibleDescriptionTimeoutMs,
    }) != UiPresentationValidation.DescriptionlessTarget)
{
    throw new InvalidDataException("presentation operation validation diverged");
}

Console.WriteLine(
    $"renderer conformance valid: revision={renderer.Document.Revision}, presentation_focus=true, presentation_navigate_focus=true, presentation_navigate_focus_first_last=true, presentation_scroll_into_view=true, presentation_assert_visible=true, presentation_assert_hidden=true, presentation_wait_hidden=true, presentation_assert_realized=true, presentation_wait_realized=true, presentation_wait_visible=true, presentation_wait_enabled=true, presentation_wait_disabled=true, presentation_assert_window_open=true, presentation_wait_window_open=true, presentation_assert_window_closed=true, presentation_wait_window_closed=true, presentation_wait_focused=true, presentation_assert_focused=true, presentation_wait_unfocused=true, presentation_assert_unfocused=true, presentation_assert_enabled=true, presentation_assert_disabled=true, presentation_assert_selection=true, presentation_wait_selection=true, presentation_assert_text=true, presentation_wait_text=true, presentation_assert_automation_id=true, presentation_assert_node_kind=true, presentation_wait_node_kind=true, presentation_assert_action_kind=true, presentation_wait_action_kind=true, presentation_assert_action_label=true, presentation_wait_action_label=true, presentation_assert_action_available=true, presentation_wait_action_available=true, presentation_assert_action_unavailable_reason=true, presentation_wait_action_unavailable_reason=true, presentation_assert_form_field=true, presentation_assert_form_field_input_kind=true, presentation_assert_form_field_required=true, presentation_assert_form_field_max_length=true, presentation_assert_form_field_placeholder=true, presentation_wait_form_field_placeholder=true, presentation_assert_accessible_name=true, presentation_wait_accessible_name=true, presentation_assert_accessible_description=true, presentation_wait_accessible_description=true, strict_codec=true");
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
