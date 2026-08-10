using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

public sealed class SemanticRenderer
{
    private const int MaxPatchOperations = 8192;
    private const int MaxFormValueBytes = 256;
    private const int MaxUiChildCount = 4096;
    private const int MaxAdapterFrameworkBytes = 128;
    private const int AdapterManifestSchemaVersion = 2;
    private const int UiSchemaVersion = 1;
    private static readonly UiPresentationAtom[] RequiredPresentationAtoms =
    [
        UiPresentationAtom.Activate,
        UiPresentationAtom.Focus,
        UiPresentationAtom.NavigateFocus,
        UiPresentationAtom.ScrollIntoView,
        UiPresentationAtom.AssertVisible,
        UiPresentationAtom.AssertHidden,
        UiPresentationAtom.WaitHidden,
        UiPresentationAtom.AssertRealized,
        UiPresentationAtom.WaitRealized,
        UiPresentationAtom.WaitVisible,
        UiPresentationAtom.WaitEnabled,
        UiPresentationAtom.WaitFocused,
        UiPresentationAtom.AssertFocused,
        UiPresentationAtom.WaitUnfocused,
        UiPresentationAtom.AssertUnfocused,
        UiPresentationAtom.AssertEnabled,
        UiPresentationAtom.AssertDisabled,
        UiPresentationAtom.WaitDisabled,
        UiPresentationAtom.OpenWindow,
        UiPresentationAtom.CloseWindow,
        UiPresentationAtom.AssertWindowOpen,
        UiPresentationAtom.WaitWindowOpen,
        UiPresentationAtom.AssertWindowClosed,
        UiPresentationAtom.WaitWindowClosed,
        UiPresentationAtom.SetSelection,
        UiPresentationAtom.AssertSelection,
        UiPresentationAtom.WaitSelection,
        UiPresentationAtom.AssertChildCount,
        UiPresentationAtom.WaitChildCount,
        UiPresentationAtom.AssertText,
        UiPresentationAtom.WaitText,
        UiPresentationAtom.AssertAutomationId,
        UiPresentationAtom.WaitAutomationId,
        UiPresentationAtom.AssertNodeKind,
        UiPresentationAtom.WaitNodeKind,
        UiPresentationAtom.AssertActionKind,
        UiPresentationAtom.WaitActionKind,
        UiPresentationAtom.AssertActionLabel,
        UiPresentationAtom.WaitActionLabel,
        UiPresentationAtom.AssertActionAvailable,
        UiPresentationAtom.WaitActionAvailable,
        UiPresentationAtom.AssertActionUnavailableReason,
        UiPresentationAtom.WaitActionUnavailableReason,
        UiPresentationAtom.SubmitForm,
        UiPresentationAtom.CancelForm,
        UiPresentationAtom.AssertFormField,
        UiPresentationAtom.WaitFormField,
        UiPresentationAtom.AssertFormFieldInputKind,
        UiPresentationAtom.WaitFormFieldInputKind,
        UiPresentationAtom.AssertFormFieldRequired,
        UiPresentationAtom.WaitFormFieldRequired,
        UiPresentationAtom.AssertFormFieldMaxLength,
        UiPresentationAtom.WaitFormFieldMaxLength,
        UiPresentationAtom.AssertFormFieldPlaceholder,
        UiPresentationAtom.WaitFormFieldPlaceholder,
        UiPresentationAtom.SetFormValue,
        UiPresentationAtom.AssertFormValue,
        UiPresentationAtom.WaitFormValue,
        UiPresentationAtom.AssertAccessibleName,
        UiPresentationAtom.WaitAccessibleName,
        UiPresentationAtom.AssertAccessibleDescription,
        UiPresentationAtom.WaitAccessibleDescription,
    ];
    public const int WaitEnabledTimeoutMs = 2000;
    public const int WaitActionAvailableTimeoutMs = 2000;
    public const int WaitActionKindTimeoutMs = 2000;
    public const int WaitActionLabelTimeoutMs = 2000;
    public const int WaitActionUnavailableReasonTimeoutMs = 2000;
    public const int WaitFocusedTimeoutMs = 2000;
    public const int WaitUnfocusedTimeoutMs = 2000;
    public const int WaitRealizedTimeoutMs = 2000;
    public const int WaitSelectionTimeoutMs = 2000;
    public const int WaitChildCountTimeoutMs = 2000;
    public const int WaitTextTimeoutMs = 2000;
    public const int WaitAccessibleNameTimeoutMs = 2000;
    public const int WaitAccessibleDescriptionTimeoutMs = 2000;
    public const int WaitFormFieldTimeoutMs = 2000;
    public const int WaitFormFieldInputKindTimeoutMs = 2000;
    public const int WaitFormFieldRequiredTimeoutMs = 2000;
    public const int WaitFormFieldMaxLengthTimeoutMs = 2000;
    public const int WaitFormFieldPlaceholderTimeoutMs = 2000;
    public const int WaitFormValueTimeoutMs = 2000;
    public const int WaitAutomationIdTimeoutMs = 2000;
    public const int WaitNodeKindTimeoutMs = 2000;
    public const int WaitVisibleTimeoutMs = 2000;
    public const int WaitWindowClosedTimeoutMs = 2000;
    public const int WaitWindowOpenTimeoutMs = 2000;

    public UiDocument Document { get; private set; } = null!;

    public void Mount(UiDocument document)
    {
        ValidateDocument(document);
        Document = Clone(document);
    }

    public static UiAdapterManifestValidation ValidateAdapterManifest(UiAdapterManifest manifest)
    {
        if (manifest.SchemaVersion != AdapterManifestSchemaVersion)
        {
            return UiAdapterManifestValidation.UnsupportedManifestSchema;
        }
        if (manifest.UiSchemaVersion != UiSchemaVersion)
        {
            return UiAdapterManifestValidation.UnsupportedUiSchema;
        }
        if (manifest.AdapterId is not { } adapterId || !IsIdentifier(adapterId))
        {
            return UiAdapterManifestValidation.InvalidAdapterId;
        }
        if (manifest.Framework is not { } framework
            || framework.Length == 0
            || Encoding.UTF8.GetByteCount(framework) > MaxAdapterFrameworkBytes
            || framework.Any(char.IsControl))
        {
            return UiAdapterManifestValidation.InvalidFramework;
        }
        if (!Enum.IsDefined(manifest.BindingKind))
        {
            return UiAdapterManifestValidation.InvalidBindingKind;
        }
        if (!manifest.DocumentSchema)
        {
            return UiAdapterManifestValidation.MissingDocumentSchema;
        }
        if (!manifest.EventSchema)
        {
            return UiAdapterManifestValidation.MissingEventSchema;
        }
        if (!manifest.PatchSchema)
        {
            return UiAdapterManifestValidation.MissingPatchSchema;
        }

        var atoms = new HashSet<UiPresentationAtom>();
        if (manifest.PresentationAtoms is null)
        {
            return UiAdapterManifestValidation.MissingPresentationAtom;
        }
        foreach (var atom in manifest.PresentationAtoms)
        {
            if (!Enum.IsDefined(atom))
            {
                return UiAdapterManifestValidation.InvalidPresentationAtom;
            }
            if (!atoms.Add(atom))
            {
                return UiAdapterManifestValidation.DuplicatePresentationAtom;
            }
        }
        if (!RequiredPresentationAtoms.All(atoms.Contains))
        {
            return UiAdapterManifestValidation.MissingPresentationAtom;
        }
        if (manifest.PresentationAtomProfiles is null)
        {
            return UiAdapterManifestValidation.MissingPresentationAtomProfile;
        }
        var profiledAtoms = new HashSet<UiPresentationAtom>();
        foreach (var profile in manifest.PresentationAtomProfiles)
        {
            if (profile is null)
            {
                return UiAdapterManifestValidation.InvalidPresentationAtomProfile;
            }
            if (!Enum.IsDefined(profile.Atom))
            {
                return UiAdapterManifestValidation.InvalidPresentationAtom;
            }
            if (!atoms.Contains(profile.Atom))
            {
                return UiAdapterManifestValidation.ProfileWithoutPresentationAtom;
            }
            if (!profiledAtoms.Add(profile.Atom))
            {
                return UiAdapterManifestValidation.DuplicatePresentationAtomProfile;
            }
            if (!Enum.IsDefined(profile.Family)
                || !Enum.IsDefined(profile.Effect)
                || profile.Family != PresentationAtomFamily(profile.Atom)
                || profile.Effect != PresentationAtomEffect(profile.Atom))
            {
                return UiAdapterManifestValidation.InvalidPresentationAtomProfile;
            }
        }
        return RequiredPresentationAtoms.All(profiledAtoms.Contains)
            ? UiAdapterManifestValidation.Valid
            : UiAdapterManifestValidation.MissingPresentationAtomProfile;
    }

    public static UiPresentationAtomProfile PresentationAtomProfile(UiPresentationAtom atom) => new()
    {
        Atom = atom,
        Family = PresentationAtomFamily(atom),
        Effect = PresentationAtomEffect(atom),
    };

    private static UiPresentationAtomFamily PresentationAtomFamily(UiPresentationAtom atom) =>
        atom switch
        {
            UiPresentationAtom.Activate => UiPresentationAtomFamily.Interaction,
            UiPresentationAtom.Focus
                or UiPresentationAtom.NavigateFocus
                or UiPresentationAtom.WaitFocused
                or UiPresentationAtom.AssertFocused
                or UiPresentationAtom.WaitUnfocused
                or UiPresentationAtom.AssertUnfocused => UiPresentationAtomFamily.Focus,
            UiPresentationAtom.ScrollIntoView => UiPresentationAtomFamily.Viewport,
            UiPresentationAtom.AssertVisible
                or UiPresentationAtom.AssertHidden
                or UiPresentationAtom.WaitHidden
                or UiPresentationAtom.WaitVisible => UiPresentationAtomFamily.Visibility,
            UiPresentationAtom.AssertRealized
                or UiPresentationAtom.WaitRealized => UiPresentationAtomFamily.Realization,
            UiPresentationAtom.WaitEnabled
                or UiPresentationAtom.AssertEnabled
                or UiPresentationAtom.AssertDisabled
                or UiPresentationAtom.WaitDisabled => UiPresentationAtomFamily.EnabledState,
            UiPresentationAtom.OpenWindow
                or UiPresentationAtom.CloseWindow
                or UiPresentationAtom.AssertWindowOpen
                or UiPresentationAtom.WaitWindowOpen
                or UiPresentationAtom.AssertWindowClosed
                or UiPresentationAtom.WaitWindowClosed => UiPresentationAtomFamily.Window,
            UiPresentationAtom.SetSelection
                or UiPresentationAtom.AssertSelection
                or UiPresentationAtom.WaitSelection => UiPresentationAtomFamily.Selection,
            UiPresentationAtom.AssertChildCount
                or UiPresentationAtom.WaitChildCount => UiPresentationAtomFamily.Structure,
            UiPresentationAtom.AssertText
                or UiPresentationAtom.WaitText => UiPresentationAtomFamily.Text,
            UiPresentationAtom.AssertAutomationId
                or UiPresentationAtom.WaitAutomationId
                or UiPresentationAtom.AssertNodeKind
                or UiPresentationAtom.WaitNodeKind => UiPresentationAtomFamily.NodeMetadata,
            UiPresentationAtom.AssertActionKind
                or UiPresentationAtom.WaitActionKind
                or UiPresentationAtom.AssertActionLabel
                or UiPresentationAtom.WaitActionLabel
                or UiPresentationAtom.AssertActionAvailable
                or UiPresentationAtom.WaitActionAvailable
                or UiPresentationAtom.AssertActionUnavailableReason
                or UiPresentationAtom.WaitActionUnavailableReason =>
                    UiPresentationAtomFamily.ActionMetadata,
            UiPresentationAtom.SubmitForm
                or UiPresentationAtom.CancelForm => UiPresentationAtomFamily.FormLifecycle,
            UiPresentationAtom.AssertFormField
                or UiPresentationAtom.WaitFormField
                or UiPresentationAtom.AssertFormFieldInputKind
                or UiPresentationAtom.WaitFormFieldInputKind
                or UiPresentationAtom.AssertFormFieldRequired
                or UiPresentationAtom.WaitFormFieldRequired
                or UiPresentationAtom.AssertFormFieldMaxLength
                or UiPresentationAtom.WaitFormFieldMaxLength
                or UiPresentationAtom.AssertFormFieldPlaceholder
                or UiPresentationAtom.WaitFormFieldPlaceholder =>
                    UiPresentationAtomFamily.FormMetadata,
            UiPresentationAtom.SetFormValue
                or UiPresentationAtom.AssertFormValue
                or UiPresentationAtom.WaitFormValue => UiPresentationAtomFamily.FormValue,
            UiPresentationAtom.AssertAccessibleName
                or UiPresentationAtom.WaitAccessibleName
                or UiPresentationAtom.AssertAccessibleDescription
                or UiPresentationAtom.WaitAccessibleDescription => UiPresentationAtomFamily.Accessibility,
            _ => throw new InvalidDataException("unknown presentation atom"),
        };

    private static UiPresentationAtomEffect PresentationAtomEffect(UiPresentationAtom atom) =>
        atom switch
        {
            UiPresentationAtom.Activate
                or UiPresentationAtom.Focus
                or UiPresentationAtom.NavigateFocus
                or UiPresentationAtom.ScrollIntoView
                or UiPresentationAtom.OpenWindow
                or UiPresentationAtom.CloseWindow
                or UiPresentationAtom.SetSelection
                or UiPresentationAtom.SubmitForm
                or UiPresentationAtom.CancelForm
                or UiPresentationAtom.SetFormValue => UiPresentationAtomEffect.Mutation,
            UiPresentationAtom.AssertVisible
                or UiPresentationAtom.AssertHidden
                or UiPresentationAtom.AssertRealized
                or UiPresentationAtom.AssertFocused
                or UiPresentationAtom.AssertUnfocused
                or UiPresentationAtom.AssertEnabled
                or UiPresentationAtom.AssertDisabled
                or UiPresentationAtom.AssertWindowOpen
                or UiPresentationAtom.AssertWindowClosed
                or UiPresentationAtom.AssertSelection
                or UiPresentationAtom.AssertChildCount
                or UiPresentationAtom.AssertText
                or UiPresentationAtom.AssertAutomationId
                or UiPresentationAtom.AssertNodeKind
                or UiPresentationAtom.AssertActionKind
                or UiPresentationAtom.AssertActionLabel
                or UiPresentationAtom.AssertActionAvailable
                or UiPresentationAtom.AssertActionUnavailableReason
                or UiPresentationAtom.AssertFormField
                or UiPresentationAtom.AssertFormFieldInputKind
                or UiPresentationAtom.AssertFormFieldRequired
                or UiPresentationAtom.AssertFormFieldMaxLength
                or UiPresentationAtom.AssertFormFieldPlaceholder
                or UiPresentationAtom.AssertFormValue
                or UiPresentationAtom.AssertAccessibleName
                or UiPresentationAtom.AssertAccessibleDescription => UiPresentationAtomEffect.Assertion,
            UiPresentationAtom.WaitHidden
                or UiPresentationAtom.WaitRealized
                or UiPresentationAtom.WaitVisible
                or UiPresentationAtom.WaitEnabled
                or UiPresentationAtom.WaitFocused
                or UiPresentationAtom.WaitUnfocused
                or UiPresentationAtom.WaitDisabled
                or UiPresentationAtom.WaitWindowOpen
                or UiPresentationAtom.WaitWindowClosed
                or UiPresentationAtom.WaitSelection
                or UiPresentationAtom.WaitChildCount
                or UiPresentationAtom.WaitText
                or UiPresentationAtom.WaitAutomationId
                or UiPresentationAtom.WaitNodeKind
                or UiPresentationAtom.WaitActionKind
                or UiPresentationAtom.WaitActionLabel
                or UiPresentationAtom.WaitActionAvailable
                or UiPresentationAtom.WaitActionUnavailableReason
                or UiPresentationAtom.WaitFormField
                or UiPresentationAtom.WaitFormFieldInputKind
                or UiPresentationAtom.WaitFormFieldRequired
                or UiPresentationAtom.WaitFormFieldMaxLength
                or UiPresentationAtom.WaitFormFieldPlaceholder
                or UiPresentationAtom.WaitFormValue
                or UiPresentationAtom.WaitAccessibleName
                or UiPresentationAtom.WaitAccessibleDescription => UiPresentationAtomEffect.Wait,
            _ => throw new InvalidDataException("unknown presentation atom"),
        };

    public void Apply(UiPatch patch)
    {
        if (patch.SchemaVersion != 1 || patch.FromRevision != Document.Revision)
        {
            throw new InvalidDataException("patch revision or schema mismatch");
        }
        if (patch.ToRevision < patch.FromRevision || patch.Operations.Count > MaxPatchOperations)
        {
            throw new InvalidDataException("invalid patch bounds");
        }

        var previous = Document;
        Document = Clone(Document);
        try
        {
            foreach (var operation in patch.Operations)
            {
                ApplyOperation(operation);
            }
            Document.Revision = patch.ToRevision;
            ValidateDocument(Document);
        }
        catch
        {
            Document = previous;
            throw;
        }
    }

    public UiEvent CreateFormSubmission(
        string nodeId,
        IReadOnlyDictionary<string, string> values)
    {
        var node = Find(Document.Root, nodeId)
            ?? throw new InvalidDataException("form event target was not found");
        var form = node.Action is { Kind: ActionKind.RuntimeDeploy, Form: not null }
            ? node.Action.Form
            : throw new InvalidDataException("form event target has no parameterized action");
        if (values.Keys.Any(key => form.Fields.All(field => field.Key != key)))
        {
            throw new InvalidDataException("form event contains an unknown field");
        }
        foreach (var field in form.Fields)
        {
            var value = values.TryGetValue(field.Key, out var provided)
                ? provided
                : string.Empty;
            if (!ValidFormValue(value, field))
            {
                throw new InvalidDataException($"form field '{field.Key}' is invalid");
            }
        }
        return new UiEvent
        {
            NodeId = nodeId,
            Kind = UiEventKind.Submit,
            Values = values.ToDictionary(
                entry => entry.Key,
                entry => entry.Value,
                StringComparer.Ordinal),
        };
    }

    public UiPresentationValidation ValidatePresentationOperation(
        UiPresentationOperation operation)
    {
        if (!IsIdentifier(operation.NodeId))
        {
            return UiPresentationValidation.UnknownTarget;
        }
        if (operation.Kind is UiPresentationOperationKind.AssertText
            or UiPresentationOperationKind.WaitText
            or UiPresentationOperationKind.AssertActionLabel
            or UiPresentationOperationKind.WaitActionLabel
            or UiPresentationOperationKind.AssertFormField
            or UiPresentationOperationKind.WaitFormField
            or UiPresentationOperationKind.AssertAccessibleName
            or UiPresentationOperationKind.WaitAccessibleName
            or UiPresentationOperationKind.AssertAccessibleDescription
            or UiPresentationOperationKind.WaitAccessibleDescription)
        {
            if (!IsExpectedText(operation.Expected))
            {
                return UiPresentationValidation.InvalidExpectedText;
            }
        }
        else if (operation.Kind is UiPresentationOperationKind.AssertAutomationId
            or UiPresentationOperationKind.WaitAutomationId)
        {
            if (operation.Expected is not { } expected
                || !IsIdentifier(expected))
            {
                return UiPresentationValidation.InvalidExpectedText;
            }
        }
        else if (operation.Kind == UiPresentationOperationKind.AssertNodeKind)
        {
            if (operation.Expected is not null)
            {
                return UiPresentationValidation.InvalidExpectedText;
            }
            if (operation.ExpectedKind is null)
            {
                return UiPresentationValidation.InvalidExpectedKind;
            }
        }
        else if (operation.Kind == UiPresentationOperationKind.WaitNodeKind)
        {
            if (operation.Expected is not null)
            {
                return UiPresentationValidation.InvalidExpectedText;
            }
            if (operation.ExpectedKind is null)
            {
                return UiPresentationValidation.InvalidExpectedKind;
            }
        }
        else if (operation.Kind == UiPresentationOperationKind.AssertActionKind)
        {
            if (operation.Expected is not null)
            {
                return UiPresentationValidation.InvalidExpectedText;
            }
            if (operation.ExpectedKind is not null)
            {
                return UiPresentationValidation.InvalidExpectedKind;
            }
            if (operation.ExpectedActionKind is null)
            {
                return UiPresentationValidation.InvalidExpectedActionKind;
            }
        }
        else if (operation.Kind == UiPresentationOperationKind.WaitActionKind)
        {
            if (operation.Expected is not null)
            {
                return UiPresentationValidation.InvalidExpectedText;
            }
            if (operation.ExpectedKind is not null)
            {
                return UiPresentationValidation.InvalidExpectedKind;
            }
            if (operation.ExpectedActionKind is null)
            {
                return UiPresentationValidation.InvalidExpectedActionKind;
            }
        }
        else if (operation.Kind is UiPresentationOperationKind.AssertActionUnavailableReason
            or UiPresentationOperationKind.WaitActionUnavailableReason
            or UiPresentationOperationKind.AssertFormFieldPlaceholder
            or UiPresentationOperationKind.WaitFormFieldPlaceholder)
        {
            if (operation.Expected is not null && !IsExpectedText(operation.Expected))
            {
                return UiPresentationValidation.InvalidExpectedText;
            }
        }
        else if (operation.Kind is UiPresentationOperationKind.AssertFormValue
            or UiPresentationOperationKind.WaitFormValue)
        {
            if (!IsFormValueText(operation.Expected))
            {
                return UiPresentationValidation.InvalidExpectedText;
            }
        }
        else if (operation.Expected is not null)
        {
            return UiPresentationValidation.InvalidExpectedText;
        }
        if (operation.Kind == UiPresentationOperationKind.SetFormValue)
        {
            if (!IsFormValueText(operation.Value))
            {
                return UiPresentationValidation.InvalidFormValue;
            }
        }
        else if (operation.Value is not null)
        {
            return UiPresentationValidation.InvalidFormValue;
        }
        if (operation.Kind is UiPresentationOperationKind.AssertFormField
            or UiPresentationOperationKind.WaitFormField
            or UiPresentationOperationKind.SetFormValue
            or UiPresentationOperationKind.AssertFormValue
            or UiPresentationOperationKind.WaitFormValue)
        {
            if (!IsFormFieldKey(operation.Field))
            {
                return UiPresentationValidation.InvalidExpectedText;
            }
        }
        else if (operation.Kind is UiPresentationOperationKind.AssertFormFieldInputKind
            or UiPresentationOperationKind.WaitFormFieldInputKind)
        {
            if (!IsFormFieldKey(operation.Field))
            {
                return UiPresentationValidation.InvalidExpectedText;
            }
            if (operation.InputKind is null)
            {
                return UiPresentationValidation.InvalidExpectedInputKind;
            }
        }
        else if (operation.Kind is UiPresentationOperationKind.AssertFormFieldRequired
            or UiPresentationOperationKind.WaitFormFieldRequired)
        {
            if (!IsFormFieldKey(operation.Field))
            {
                return UiPresentationValidation.InvalidExpectedText;
            }
            if (operation.Required is null)
            {
                return UiPresentationValidation.InvalidExpectedRequired;
            }
        }
        else if (operation.Kind is UiPresentationOperationKind.AssertFormFieldMaxLength
            or UiPresentationOperationKind.WaitFormFieldMaxLength)
        {
            if (!IsFormFieldKey(operation.Field))
            {
                return UiPresentationValidation.InvalidExpectedText;
            }
            if (operation.MaxLength is not { } maxLength
                || maxLength is < 1 or > MaxFormValueBytes)
            {
                return UiPresentationValidation.InvalidExpectedMaxLength;
            }
        }
        else if (operation.Kind is UiPresentationOperationKind.AssertFormFieldPlaceholder
            or UiPresentationOperationKind.WaitFormFieldPlaceholder)
        {
            if (!IsFormFieldKey(operation.Field))
            {
                return UiPresentationValidation.InvalidExpectedText;
            }
        }
        else if (operation.Field is not null)
        {
            return UiPresentationValidation.InvalidExpectedText;
        }
        if (operation.Kind != UiPresentationOperationKind.AssertFormFieldInputKind
            && operation.Kind != UiPresentationOperationKind.WaitFormFieldInputKind
            && operation.InputKind is not null)
        {
            return UiPresentationValidation.InvalidExpectedInputKind;
        }
        if (operation.Kind != UiPresentationOperationKind.AssertFormFieldRequired
            && operation.Kind != UiPresentationOperationKind.WaitFormFieldRequired
            && operation.Required is not null)
        {
            return UiPresentationValidation.InvalidExpectedRequired;
        }
        if (operation.Kind != UiPresentationOperationKind.AssertFormFieldMaxLength
            && operation.Kind != UiPresentationOperationKind.WaitFormFieldMaxLength
            && operation.MaxLength is not null)
        {
            return UiPresentationValidation.InvalidExpectedMaxLength;
        }
        if (operation.Kind != UiPresentationOperationKind.AssertNodeKind
            && operation.Kind != UiPresentationOperationKind.WaitNodeKind
            && operation.ExpectedKind is not null)
        {
            return UiPresentationValidation.InvalidExpectedKind;
        }
        if (operation.Kind != UiPresentationOperationKind.AssertActionKind
            && operation.Kind != UiPresentationOperationKind.WaitActionKind
            && operation.ExpectedActionKind is not null)
        {
            return UiPresentationValidation.InvalidExpectedActionKind;
        }
        if (operation.Kind == UiPresentationOperationKind.NavigateFocus)
        {
            if (operation.Direction is null)
            {
                return UiPresentationValidation.InvalidNavigationDirection;
            }
        }
        else if (operation.Direction is not null)
        {
            return UiPresentationValidation.InvalidNavigationDirection;
        }
        if (operation.Kind is UiPresentationOperationKind.SetSelection
            or UiPresentationOperationKind.AssertSelection
            or UiPresentationOperationKind.WaitSelection)
        {
            if (operation.State is null)
            {
                return UiPresentationValidation.InvalidSelectionState;
            }
        }
        else if (operation.State is not null)
        {
            return UiPresentationValidation.InvalidSelectionState;
        }
        if (operation.Kind is UiPresentationOperationKind.AssertChildCount
            or UiPresentationOperationKind.WaitChildCount)
        {
            if (operation.Count is not { } count || count is < 0 or > MaxUiChildCount)
            {
                return UiPresentationValidation.InvalidExpectedChildCount;
            }
        }
        else if (operation.Count is not null)
        {
            return UiPresentationValidation.InvalidExpectedChildCount;
        }
        if (operation.Kind is UiPresentationOperationKind.WaitRealized
            or UiPresentationOperationKind.WaitVisible
            or UiPresentationOperationKind.WaitHidden
            or UiPresentationOperationKind.WaitEnabled
            or UiPresentationOperationKind.WaitDisabled
            or UiPresentationOperationKind.WaitActionAvailable
            or UiPresentationOperationKind.WaitActionKind
            or UiPresentationOperationKind.WaitActionLabel
            or UiPresentationOperationKind.WaitActionUnavailableReason
            or UiPresentationOperationKind.WaitAutomationId
            or UiPresentationOperationKind.WaitNodeKind
            or UiPresentationOperationKind.WaitWindowOpen
            or UiPresentationOperationKind.WaitWindowClosed
            or UiPresentationOperationKind.WaitFocused
            or UiPresentationOperationKind.WaitUnfocused
            or UiPresentationOperationKind.WaitSelection
            or UiPresentationOperationKind.WaitChildCount
            or UiPresentationOperationKind.WaitText
            or UiPresentationOperationKind.WaitAccessibleName
            or UiPresentationOperationKind.WaitAccessibleDescription
            or UiPresentationOperationKind.WaitFormField
            or UiPresentationOperationKind.WaitFormFieldInputKind
            or UiPresentationOperationKind.WaitFormFieldRequired
            or UiPresentationOperationKind.WaitFormFieldMaxLength
            or UiPresentationOperationKind.WaitFormFieldPlaceholder
            or UiPresentationOperationKind.WaitFormValue)
        {
            var requiredTimeout = operation.Kind switch
            {
                UiPresentationOperationKind.WaitRealized => WaitRealizedTimeoutMs,
                UiPresentationOperationKind.WaitVisible => WaitVisibleTimeoutMs,
                UiPresentationOperationKind.WaitHidden => WaitVisibleTimeoutMs,
                UiPresentationOperationKind.WaitEnabled => WaitEnabledTimeoutMs,
                UiPresentationOperationKind.WaitDisabled => WaitEnabledTimeoutMs,
                UiPresentationOperationKind.WaitActionAvailable =>
                    WaitActionAvailableTimeoutMs,
                UiPresentationOperationKind.WaitActionKind =>
                    WaitActionKindTimeoutMs,
                UiPresentationOperationKind.WaitActionLabel =>
                    WaitActionLabelTimeoutMs,
                UiPresentationOperationKind.WaitActionUnavailableReason =>
                    WaitActionUnavailableReasonTimeoutMs,
                UiPresentationOperationKind.WaitAutomationId => WaitAutomationIdTimeoutMs,
                UiPresentationOperationKind.WaitNodeKind => WaitNodeKindTimeoutMs,
                UiPresentationOperationKind.WaitWindowOpen => WaitWindowOpenTimeoutMs,
                UiPresentationOperationKind.WaitWindowClosed => WaitWindowClosedTimeoutMs,
                UiPresentationOperationKind.WaitFocused => WaitFocusedTimeoutMs,
                UiPresentationOperationKind.WaitUnfocused => WaitUnfocusedTimeoutMs,
                UiPresentationOperationKind.WaitSelection => WaitSelectionTimeoutMs,
                UiPresentationOperationKind.WaitChildCount => WaitChildCountTimeoutMs,
                UiPresentationOperationKind.WaitText => WaitTextTimeoutMs,
                UiPresentationOperationKind.WaitAccessibleName => WaitAccessibleNameTimeoutMs,
                UiPresentationOperationKind.WaitAccessibleDescription =>
                    WaitAccessibleDescriptionTimeoutMs,
                UiPresentationOperationKind.WaitFormField => WaitFormFieldTimeoutMs,
                UiPresentationOperationKind.WaitFormFieldInputKind =>
                    WaitFormFieldInputKindTimeoutMs,
                UiPresentationOperationKind.WaitFormFieldRequired =>
                    WaitFormFieldRequiredTimeoutMs,
                UiPresentationOperationKind.WaitFormFieldMaxLength =>
                    WaitFormFieldMaxLengthTimeoutMs,
                UiPresentationOperationKind.WaitFormFieldPlaceholder =>
                    WaitFormFieldPlaceholderTimeoutMs,
                UiPresentationOperationKind.WaitFormValue => WaitFormValueTimeoutMs,
                _ => throw new InvalidOperationException("unknown wait operation"),
            };
            if (operation.TimeoutMs != requiredTimeout)
            {
                return UiPresentationValidation.InvalidTimeout;
            }
        }
        else if (operation.TimeoutMs is not null)
        {
            return UiPresentationValidation.InvalidTimeout;
        }
        var node = Find(Document.Root, operation.NodeId);
        if (node is null)
        {
            return UiPresentationValidation.UnknownTarget;
        }
        if (operation.Kind is UiPresentationOperationKind.SetFormValue
            or UiPresentationOperationKind.AssertFormValue
            or UiPresentationOperationKind.WaitFormValue)
        {
            if (node.Action?.Form is not { } form)
            {
                return UiPresentationValidation.FormlessTarget;
            }
            var field = form.Fields.FirstOrDefault(candidate =>
                StringComparer.Ordinal.Equals(candidate.Key, operation.Field));
            if (field is null)
            {
                return UiPresentationValidation.UnknownFormField;
            }
            if (operation.Kind == UiPresentationOperationKind.SetFormValue
                && !ValidFormValue(operation.Value!, field))
            {
                return UiPresentationValidation.InvalidFormValue;
            }
            return UiPresentationValidation.Valid;
        }
        if (operation.Kind is UiPresentationOperationKind.SubmitForm
            or UiPresentationOperationKind.CancelForm)
        {
            return node.Action?.Form is not null
                ? UiPresentationValidation.Valid
                : UiPresentationValidation.FormlessTarget;
        }
        return operation.Kind switch
        {
            UiPresentationOperationKind.Activate
            or UiPresentationOperationKind.Focus
            or UiPresentationOperationKind.NavigateFocus
            or UiPresentationOperationKind.AssertFocused
            or UiPresentationOperationKind.AssertUnfocused
            or UiPresentationOperationKind.AssertEnabled
            or UiPresentationOperationKind.AssertDisabled
            or UiPresentationOperationKind.WaitEnabled
            or UiPresentationOperationKind.WaitDisabled
            or UiPresentationOperationKind.WaitFocused
            or UiPresentationOperationKind.WaitUnfocused
            or UiPresentationOperationKind.AssertActionKind
            or UiPresentationOperationKind.WaitActionKind
            or UiPresentationOperationKind.AssertActionLabel
            or UiPresentationOperationKind.WaitActionLabel
            or UiPresentationOperationKind.AssertActionAvailable
            or UiPresentationOperationKind.WaitActionAvailable
            or UiPresentationOperationKind.AssertActionUnavailableReason
            or UiPresentationOperationKind.WaitActionUnavailableReason
                when node.Kind == UiNodeKind.Action && node.Action is not null =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.ScrollIntoView =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.AssertVisible =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.AssertHidden =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.WaitHidden =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.AssertRealized =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.WaitRealized =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.WaitVisible =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.OpenWindow =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.CloseWindow =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.AssertWindowOpen =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.WaitWindowOpen =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.AssertWindowClosed =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.WaitWindowClosed =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.SetSelection
            or UiPresentationOperationKind.AssertSelection
            or UiPresentationOperationKind.WaitSelection
                when node.Selection is not null =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.AssertChildCount
            or UiPresentationOperationKind.WaitChildCount =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.AssertText
            or UiPresentationOperationKind.WaitText
                when node.Text is not null
                    && node.Kind is UiNodeKind.Heading
                        or UiNodeKind.Text
                        or UiNodeKind.HistoryEntry
                        or UiNodeKind.LogEntry
                        or UiNodeKind.DebuggerFrame
                        or UiNodeKind.Action =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.AssertAccessibleName
            or UiPresentationOperationKind.WaitAccessibleName =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.AssertAutomationId
            or UiPresentationOperationKind.WaitAutomationId =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.AssertNodeKind
            or UiPresentationOperationKind.WaitNodeKind =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.AssertFormField
            or UiPresentationOperationKind.WaitFormField
            or UiPresentationOperationKind.AssertFormFieldInputKind
            or UiPresentationOperationKind.WaitFormFieldInputKind
            or UiPresentationOperationKind.AssertFormFieldRequired
            or UiPresentationOperationKind.WaitFormFieldRequired
            or UiPresentationOperationKind.AssertFormFieldMaxLength
            or UiPresentationOperationKind.WaitFormFieldMaxLength
            or UiPresentationOperationKind.AssertFormFieldPlaceholder
            or UiPresentationOperationKind.WaitFormFieldPlaceholder
                when node.Action?.Form is { } form
                    && form.Fields.Any(field => StringComparer.Ordinal.Equals(field.Key, operation.Field)) =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.AssertFormField
            or UiPresentationOperationKind.WaitFormField
            or UiPresentationOperationKind.AssertFormFieldInputKind
            or UiPresentationOperationKind.WaitFormFieldInputKind
            or UiPresentationOperationKind.AssertFormFieldRequired
            or UiPresentationOperationKind.WaitFormFieldRequired
            or UiPresentationOperationKind.AssertFormFieldMaxLength
            or UiPresentationOperationKind.WaitFormFieldMaxLength
            or UiPresentationOperationKind.AssertFormFieldPlaceholder
            or UiPresentationOperationKind.WaitFormFieldPlaceholder
                when node.Action?.Form is not null =>
                UiPresentationValidation.UnknownFormField,
            UiPresentationOperationKind.AssertFormField =>
                UiPresentationValidation.FormlessTarget,
            UiPresentationOperationKind.WaitFormField =>
                UiPresentationValidation.FormlessTarget,
            UiPresentationOperationKind.AssertFormFieldInputKind =>
                UiPresentationValidation.FormlessTarget,
            UiPresentationOperationKind.WaitFormFieldInputKind =>
                UiPresentationValidation.FormlessTarget,
            UiPresentationOperationKind.AssertFormFieldRequired =>
                UiPresentationValidation.FormlessTarget,
            UiPresentationOperationKind.WaitFormFieldRequired =>
                UiPresentationValidation.FormlessTarget,
            UiPresentationOperationKind.AssertFormFieldMaxLength =>
                UiPresentationValidation.FormlessTarget,
            UiPresentationOperationKind.WaitFormFieldMaxLength =>
                UiPresentationValidation.FormlessTarget,
            UiPresentationOperationKind.AssertFormFieldPlaceholder =>
                UiPresentationValidation.FormlessTarget,
            UiPresentationOperationKind.WaitFormFieldPlaceholder =>
                UiPresentationValidation.FormlessTarget,
            UiPresentationOperationKind.AssertAccessibleDescription
            or UiPresentationOperationKind.WaitAccessibleDescription
                when node.Accessibility.Description is not null =>
                UiPresentationValidation.Valid,
            UiPresentationOperationKind.AssertAccessibleDescription
            or UiPresentationOperationKind.WaitAccessibleDescription =>
                UiPresentationValidation.DescriptionlessTarget,
            UiPresentationOperationKind.SetSelection
            or UiPresentationOperationKind.AssertSelection
            or UiPresentationOperationKind.WaitSelection =>
                UiPresentationValidation.SelectionlessTarget,
            UiPresentationOperationKind.AssertText
            or UiPresentationOperationKind.WaitText =>
                UiPresentationValidation.TextlessTarget,
            _ => UiPresentationValidation.UnfocusableTarget,
        };
    }

    private static bool IsExpectedText(string? value) =>
        value is not null
        && Encoding.UTF8.GetByteCount(value) <= 1024
        && !value.Any(char.IsControl);

    private static bool IsFormValueText(string? value) =>
        value is not null
        && Encoding.UTF8.GetByteCount(value) <= MaxFormValueBytes
        && !value.Any(char.IsControl);

    private static bool IsFormFieldKey(string? value) =>
        value is { Length: > 0 and <= 128 }
        && value.All(character => char.IsAsciiLetterOrDigit(character)
            || character is '-' or '_' or '.');

    private void ApplyOperation(UiPatchOperation operation)
    {
        switch (operation.Kind)
        {
            case PatchKind.Remove:
                Require(operation.NodeId is not null, "remove target is missing");
                Require(operation.ParentId is null && operation.Index is null && operation.Node is null,
                    "remove contains unrelated fields");
                Require(operation.NodeId != Document.Root.Id, "root cannot be removed");
                Require(Remove(Document.Root, operation.NodeId!) is not null, "remove target not found");
                break;
            case PatchKind.Insert:
                Require(operation.ParentId is not null && operation.Index is not null && operation.Node is not null,
                    "insert payload is incomplete");
                Require(operation.NodeId is null, "insert contains unrelated fields");
                var insertIndex = operation.Index
                    ?? throw new InvalidDataException("insert index is missing");
                Require(Find(Document.Root, operation.Node!.Id) is null, "insert ID already exists");
                var insertParent = Find(Document.Root, operation.ParentId!)
                    ?? throw new InvalidDataException("insert parent not found");
                Require(insertIndex <= insertParent.Children.Count, "insert index is invalid");
                insertParent.Children.Insert(insertIndex, Clone(operation.Node));
                break;
            case PatchKind.Move:
                Require(operation.NodeId is not null && operation.ParentId is not null && operation.Index is not null,
                    "move payload is incomplete");
                Require(operation.Node is null, "move contains unrelated fields");
                var moveIndex = operation.Index
                    ?? throw new InvalidDataException("move index is missing");
                var moving = Find(Document.Root, operation.NodeId!)
                    ?? throw new InvalidDataException("move target not found");
                Require(Find(moving, operation.ParentId!) is null, "cyclic move");
                moving = Remove(Document.Root, operation.NodeId!)
                    ?? throw new InvalidDataException("move target disappeared");
                var moveParent = Find(Document.Root, operation.ParentId!)
                    ?? throw new InvalidDataException("move parent not found");
                Require(moveIndex <= moveParent.Children.Count, "move index is invalid");
                moveParent.Children.Insert(moveIndex, moving);
                break;
            case PatchKind.Update:
                Require(operation.Node is not null && operation.Node.Children.Count == 0,
                    "update must contain one shallow node");
                Require(operation.NodeId is null && operation.ParentId is null && operation.Index is null,
                    "update contains unrelated fields");
                var target = Find(Document.Root, operation.Node!.Id)
                    ?? throw new InvalidDataException("update target not found");
                target.Kind = operation.Node.Kind;
                target.RuntimeId = operation.Node.RuntimeId;
                target.DebuggerSessionId = operation.Node.DebuggerSessionId;
                target.Text = operation.Node.Text;
                target.Accessibility = operation.Node.Accessibility;
                target.Selection = operation.Node.Selection;
                target.Action = operation.Node.Action;
                break;
            default:
                throw new InvalidDataException("unknown patch operation");
        }
    }

    private static void ValidateDocument(UiDocument document)
    {
        Require(document.SchemaVersion == 1, "unsupported document schema");
        var ids = new HashSet<string>(StringComparer.Ordinal);
        ValidateNode(document.Root, 1, null, null, ids);
        Require(ids.Count <= 4096, "document exceeds the node limit");
    }

    private static void ValidateNode(
        UiNode node,
        int depth,
        string? runtimeContext,
        string? debuggerContext,
        HashSet<string> ids)
    {
        Require(depth <= 32, "document exceeds the depth limit");
        Require(IsIdentifier(node.Id) && ids.Add(node.Id), "invalid or duplicate node ID");
        ValidateText(node.Text);
        ValidateText(node.Accessibility.Label);
        ValidateText(node.Accessibility.Description);
        Require(node.Action is null || node.Accessibility.Label is not null, "action has no accessibility label");
        if (node.Kind is UiNodeKind.RuntimeCard or UiNodeKind.RuntimeWorkspace)
        {
            Require(node.RuntimeId is not null, "runtime container has no runtime ID");
            runtimeContext = node.RuntimeId;
        }
        else
        {
            Require(node.RuntimeId is null, "non-container node carries a runtime ID");
        }
        if (node.Kind is UiNodeKind.DebuggerWorkspace)
        {
            Require(node.DebuggerSessionId is not null && IsIdentifier(node.DebuggerSessionId),
                "debugger workspace has no valid session ID");
            debuggerContext = node.DebuggerSessionId;
        }
        else
        {
            Require(node.DebuggerSessionId is null,
                "non-debugger container carries a debugger session ID");
        }
        if (node.Action is not null)
        {
            var validAction = node.Action.Kind switch
            {
                ActionKind.RuntimeInspect
                    or ActionKind.RuntimeRefresh
                    or ActionKind.RuntimeCapabilitiesRefresh =>
                    node.Action.RuntimeId is not null
                    && IsIdentifier(node.Action.RuntimeId)
                    && node.Action.RuntimeId == runtimeContext
                    && node.Action.SessionId is null
                    && node.Action.Form is null,
                ActionKind.RuntimeDeploy => node.Action.RuntimeId is not null
                    && IsIdentifier(node.Action.RuntimeId)
                    && node.Action.RuntimeId == runtimeContext
                    && node.Action.SessionId is null
                    && ValidForm(node.Action.Form),
                ActionKind.DebuggerCancel => node.Action.RuntimeId is null
                    && node.Action.SessionId is not null
                    && IsIdentifier(node.Action.SessionId)
                    && node.Action.SessionId == debuggerContext
                    && node.Action.Form is null,
                _ => false,
            };
            Require(validAction, "action context binding is invalid");
        }
        foreach (var child in node.Children)
        {
            ValidateNode(child, depth + 1, runtimeContext, debuggerContext, ids);
        }
    }

    private static void ValidateText(LocalizedText? text)
    {
        if (text is null) return;
        Require(IsIdentifier(text.Key) && text.Fallback.Length <= 1024
            && !text.Fallback.Any(char.IsControl), "invalid localized text");
    }

    private static bool ValidForm(UiForm? form)
    {
        if (form is null || form.Fields.Count is < 1 or > 16)
        {
            return false;
        }
        ValidateText(form.Title);
        ValidateText(form.SubmitLabel);
        var keys = new HashSet<string>(StringComparer.Ordinal);
        foreach (var field in form.Fields)
        {
            ValidateText(field.Label);
            ValidateText(field.Placeholder);
            if (!IsIdentifier(field.Key)
                || !keys.Add(field.Key)
                || field.MaxLength is < 1 or > MaxFormValueBytes)
            {
                return false;
            }
        }
        return true;
    }

    private static bool ValidFormValue(string value, UiFormField field)
    {
        if ((field.Required && value.Length == 0)
            || Encoding.UTF8.GetByteCount(value) > field.MaxLength
            || Encoding.UTF8.GetByteCount(value) > MaxFormValueBytes)
        {
            return false;
        }
        return field.InputKind switch
        {
            UiFormInputKind.PathToken => value.Length > 0
                && value.All(character => char.IsAsciiLetterOrDigit(character)
                    || character is '.' or '/' or '_' or '-'),
            UiFormInputKind.TrimmedText => value == value.Trim()
                && !value.Any(char.IsControl),
            _ => false,
        };
    }

    private static bool IsIdentifier(string value) => value.Length is > 0 and <= 128
        && value.All(character => char.IsAsciiLetterOrDigit(character)
            || character is '-' or '_' or '.' or ':');

    private static UiNode? Find(UiNode node, string id)
    {
        if (node.Id == id) return node;
        foreach (var child in node.Children)
        {
            var found = Find(child, id);
            if (found is not null) return found;
        }
        return null;
    }

    private static UiNode? Remove(UiNode node, string id)
    {
        var index = node.Children.FindIndex(child => child.Id == id);
        if (index >= 0)
        {
            var removed = node.Children[index];
            node.Children.RemoveAt(index);
            return removed;
        }
        foreach (var child in node.Children)
        {
            var removed = Remove(child, id);
            if (removed is not null) return removed;
        }
        return null;
    }

    private static UiDocument Clone(UiDocument document) => new()
    {
        SchemaVersion = document.SchemaVersion,
        Revision = document.Revision,
        Root = Clone(document.Root),
    };

    private static UiNode Clone(UiNode node) => new()
    {
        Id = node.Id,
        Kind = node.Kind,
        RuntimeId = node.RuntimeId,
        DebuggerSessionId = node.DebuggerSessionId,
        Text = Clone(node.Text),
        Accessibility = new Accessibility
        {
            Label = Clone(node.Accessibility.Label),
            Description = Clone(node.Accessibility.Description),
        },
        Selection = node.Selection is null ? null : new UiSelection
        {
            State = node.Selection.State,
        },
        Action = node.Action is null ? null : new UiAction
        {
            Kind = node.Action.Kind,
            RuntimeId = node.Action.RuntimeId,
            SessionId = node.Action.SessionId,
            Form = Clone(node.Action.Form),
        },
        Children = node.Children.Select(Clone).ToList(),
    };

    private static LocalizedText? Clone(LocalizedText? text) => text is null ? null : new()
    {
        Key = text.Key,
        Fallback = text.Fallback,
    };

    private static UiForm? Clone(UiForm? form) => form is null ? null : new()
    {
        Title = Clone(form.Title)!,
        SubmitLabel = Clone(form.SubmitLabel)!,
        Fields = form.Fields.Select(field => new UiFormField
        {
            Key = field.Key,
            Label = Clone(field.Label)!,
            Placeholder = Clone(field.Placeholder),
            Required = field.Required,
            MaxLength = field.MaxLength,
            InputKind = field.InputKind,
        }).ToList(),
    };

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidDataException(message);
    }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RendererFixture
{
    public int SchemaVersion { get; set; }
    public required UiDocument Previous { get; set; }
    public required UiPatch Patch { get; set; }
    public required UiDocument Next { get; set; }
    public UiPresentationOperation? ActivateOperation { get; set; }
    public UiPresentationOperation? PresentationOperation { get; set; }
    public UiPresentationOperation? NavigationOperation { get; set; }
    public UiPresentationOperation? NavigationFirstOperation { get; set; }
    public UiPresentationOperation? NavigationLastOperation { get; set; }
    public UiPresentationOperation? ScrollOperation { get; set; }
    public UiPresentationOperation? AssertOperation { get; set; }
    public UiPresentationOperation? HiddenAssertOperation { get; set; }
    public UiPresentationOperation? HiddenWaitOperation { get; set; }
    public UiPresentationOperation? RealizedAssertOperation { get; set; }
    public UiPresentationOperation? RealizedWaitOperation { get; set; }
    public UiPresentationOperation? VisibleWaitOperation { get; set; }
    public UiPresentationOperation? EnabledWaitOperation { get; set; }
    public UiPresentationOperation? DisabledWaitOperation { get; set; }
    public UiPresentationOperation? WindowOpenOperation { get; set; }
    public UiPresentationOperation? WindowCloseOperation { get; set; }
    public UiPresentationOperation? WindowOpenAssertOperation { get; set; }
    public UiPresentationOperation? WindowOpenWaitOperation { get; set; }
    public UiPresentationOperation? WindowClosedAssertOperation { get; set; }
    public UiPresentationOperation? WindowClosedWaitOperation { get; set; }
    public UiPresentationOperation? FocusedWaitOperation { get; set; }
    public UiPresentationOperation? FocusedAssertOperation { get; set; }
    public UiPresentationOperation? UnfocusedWaitOperation { get; set; }
    public UiPresentationOperation? UnfocusedAssertOperation { get; set; }
    public UiPresentationOperation? EnabledAssertOperation { get; set; }
    public UiPresentationOperation? DisabledAssertOperation { get; set; }
    public UiPresentationOperation? SelectionSetOperation { get; set; }
    public UiPresentationOperation? SelectionAssertOperation { get; set; }
    public UiPresentationOperation? SelectionWaitOperation { get; set; }
    public UiPresentationOperation? ChildCountAssertOperation { get; set; }
    public UiPresentationOperation? ChildCountWaitOperation { get; set; }
    public UiPresentationOperation? TextAssertOperation { get; set; }
    public UiPresentationOperation? TextWaitOperation { get; set; }
    public UiPresentationOperation? AutomationIdAssertOperation { get; set; }
    public UiPresentationOperation? AutomationIdWaitOperation { get; set; }
    public UiPresentationOperation? NodeKindAssertOperation { get; set; }
    public UiPresentationOperation? NodeKindWaitOperation { get; set; }
    public UiPresentationOperation? ActionKindAssertOperation { get; set; }
    public UiPresentationOperation? ActionKindWaitOperation { get; set; }
    public UiPresentationOperation? ActionLabelAssertOperation { get; set; }
    public UiPresentationOperation? ActionLabelWaitOperation { get; set; }
    public UiPresentationOperation? ActionAvailableAssertOperation { get; set; }
    public UiPresentationOperation? ActionAvailableWaitOperation { get; set; }
    public UiPresentationOperation? ActionUnavailableReasonAssertOperation { get; set; }
    public UiPresentationOperation? ActionUnavailableReasonWaitOperation { get; set; }
    public UiPresentationOperation? FormFieldAssertOperation { get; set; }
    public UiPresentationOperation? FormFieldInputKindAssertOperation { get; set; }
    public UiPresentationOperation? FormFieldRequiredAssertOperation { get; set; }
    public UiPresentationOperation? FormFieldMaxLengthAssertOperation { get; set; }
    public UiPresentationOperation? FormFieldPlaceholderAssertOperation { get; set; }
    public UiPresentationOperation? FormFieldWaitOperation { get; set; }
    public UiPresentationOperation? FormFieldInputKindWaitOperation { get; set; }
    public UiPresentationOperation? FormFieldRequiredWaitOperation { get; set; }
    public UiPresentationOperation? FormFieldMaxLengthWaitOperation { get; set; }
    public UiPresentationOperation? FormFieldPlaceholderWaitOperation { get; set; }
    public UiPresentationOperation? FormSubmitOperation { get; set; }
    public UiPresentationOperation? FormCancelOperation { get; set; }
    public UiPresentationOperation? FormValueSetOperation { get; set; }
    public UiPresentationOperation? FormValueAssertOperation { get; set; }
    public UiPresentationOperation? FormValueWaitOperation { get; set; }
    public UiPresentationOperation? AccessibleNameAssertOperation { get; set; }
    public UiPresentationOperation? AccessibleNameWaitOperation { get; set; }
    public UiPresentationOperation? AccessibleDescriptionAssertOperation { get; set; }
    public UiPresentationOperation? AccessibleDescriptionWaitOperation { get; set; }
    public UiAdapterManifest? AdapterManifest { get; set; }
    public UiAdapterManifest? GeneratedAdapterManifest { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class UiAdapterManifest
{
    public int SchemaVersion { get; set; }
    public required string AdapterId { get; set; }
    public required string Framework { get; set; }
    public UiAdapterBindingKind BindingKind { get; set; }
    public int UiSchemaVersion { get; set; }
    public bool DocumentSchema { get; set; }
    public bool EventSchema { get; set; }
    public bool PatchSchema { get; set; }
    public required List<UiPresentationAtom> PresentationAtoms { get; set; }
    public required List<UiPresentationAtomProfile> PresentationAtomProfiles { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class UiPresentationAtomProfile
{
    public UiPresentationAtom Atom { get; set; }
    public UiPresentationAtomFamily Family { get; set; }
    public UiPresentationAtomEffect Effect { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class UiDocument
{
    public int SchemaVersion { get; set; }
    public ulong Revision { get; set; }
    public required UiNode Root { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class UiNode
{
    public required string Id { get; set; }
    public UiNodeKind Kind { get; set; }
    public string? RuntimeId { get; set; }
    public string? DebuggerSessionId { get; set; }
    public LocalizedText? Text { get; set; }
    public required Accessibility Accessibility { get; set; }
    public UiSelection? Selection { get; set; }
    public UiAction? Action { get; set; }
    public required List<UiNode> Children { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class LocalizedText
{
    public required string Key { get; set; }
    public required string Fallback { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class Accessibility
{
    public LocalizedText? Label { get; set; }
    public LocalizedText? Description { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class UiSelection
{
    public UiSelectionState State { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class UiAction
{
    public ActionKind Kind { get; set; }
    public string? RuntimeId { get; set; }
    public string? SessionId { get; set; }
    public UiForm? Form { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class UiForm
{
    public required LocalizedText Title { get; set; }
    public required LocalizedText SubmitLabel { get; set; }
    public required List<UiFormField> Fields { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class UiFormField
{
    public required string Key { get; set; }
    public required LocalizedText Label { get; set; }
    public LocalizedText? Placeholder { get; set; }
    public bool Required { get; set; }
    public int MaxLength { get; set; }
    public UiFormInputKind InputKind { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class UiEvent
{
    public required string NodeId { get; set; }
    public UiEventKind Kind { get; set; }
    public required Dictionary<string, string> Values { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class UiPresentationOperation
{
    public UiPresentationOperationKind Kind { get; set; }
    public required string NodeId { get; set; }
    public UiFocusNavigationDirection? Direction { get; set; }
    public UiSelectionState? State { get; set; }
    public int? Count { get; set; }
    public string? Expected { get; set; }
    public string? Field { get; set; }
    public string? Value { get; set; }
    public UiFormInputKind? InputKind { get; set; }
    public bool? Required { get; set; }
    public int? MaxLength { get; set; }
    public UiNodeKind? ExpectedKind { get; set; }
    public ActionKind? ExpectedActionKind { get; set; }
    public int? TimeoutMs { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class UiPatch
{
    public int SchemaVersion { get; set; }
    public ulong FromRevision { get; set; }
    public ulong ToRevision { get; set; }
    public required List<UiPatchOperation> Operations { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class UiPatchOperation
{
    public PatchKind Kind { get; set; }
    public string? NodeId { get; set; }
    public string? ParentId { get; set; }
    public int? Index { get; set; }
    public UiNode? Node { get; set; }
}

[JsonConverter(typeof(JsonStringEnumConverter<UiNodeKind>))]
public enum UiNodeKind
{
    [JsonStringEnumMemberName("column")] Column,
    [JsonStringEnumMemberName("heading")] Heading,
    [JsonStringEnumMemberName("text")] Text,
    [JsonStringEnumMemberName("runtime_card")] RuntimeCard,
    [JsonStringEnumMemberName("runtime_workspace")] RuntimeWorkspace,
    [JsonStringEnumMemberName("section")] Section,
    [JsonStringEnumMemberName("history_entry")] HistoryEntry,
    [JsonStringEnumMemberName("log_entry")] LogEntry,
    [JsonStringEnumMemberName("debugger_workspace")] DebuggerWorkspace,
    [JsonStringEnumMemberName("debugger_frame")] DebuggerFrame,
    [JsonStringEnumMemberName("action")] Action,
}

[JsonConverter(typeof(JsonStringEnumConverter<ActionKind>))]
public enum ActionKind
{
    [JsonStringEnumMemberName("runtime_inspect")] RuntimeInspect,
    [JsonStringEnumMemberName("runtime_refresh")] RuntimeRefresh,
    [JsonStringEnumMemberName("runtime_capabilities_refresh")] RuntimeCapabilitiesRefresh,
    [JsonStringEnumMemberName("runtime_deploy")] RuntimeDeploy,
    [JsonStringEnumMemberName("debugger_cancel")] DebuggerCancel,
}

[JsonConverter(typeof(JsonStringEnumConverter<UiFormInputKind>))]
public enum UiFormInputKind
{
    [JsonStringEnumMemberName("path_token")] PathToken,
    [JsonStringEnumMemberName("trimmed_text")] TrimmedText,
}

[JsonConverter(typeof(JsonStringEnumConverter<UiEventKind>))]
public enum UiEventKind
{
    [JsonStringEnumMemberName("activate")] Activate,
    [JsonStringEnumMemberName("submit")] Submit,
}

[JsonConverter(typeof(JsonStringEnumConverter<UiPresentationOperationKind>))]
public enum UiPresentationOperationKind
{
    [JsonStringEnumMemberName("activate")] Activate,
    [JsonStringEnumMemberName("focus")] Focus,
    [JsonStringEnumMemberName("navigate_focus")] NavigateFocus,
    [JsonStringEnumMemberName("scroll_into_view")] ScrollIntoView,
    [JsonStringEnumMemberName("assert_visible")] AssertVisible,
    [JsonStringEnumMemberName("assert_hidden")] AssertHidden,
    [JsonStringEnumMemberName("wait_hidden")] WaitHidden,
    [JsonStringEnumMemberName("assert_realized")] AssertRealized,
    [JsonStringEnumMemberName("wait_realized")] WaitRealized,
    [JsonStringEnumMemberName("wait_visible")] WaitVisible,
    [JsonStringEnumMemberName("wait_enabled")] WaitEnabled,
    [JsonStringEnumMemberName("wait_disabled")] WaitDisabled,
    [JsonStringEnumMemberName("open_window")] OpenWindow,
    [JsonStringEnumMemberName("close_window")] CloseWindow,
    [JsonStringEnumMemberName("assert_window_open")] AssertWindowOpen,
    [JsonStringEnumMemberName("wait_window_open")] WaitWindowOpen,
    [JsonStringEnumMemberName("assert_window_closed")] AssertWindowClosed,
    [JsonStringEnumMemberName("wait_window_closed")] WaitWindowClosed,
    [JsonStringEnumMemberName("wait_focused")] WaitFocused,
    [JsonStringEnumMemberName("assert_focused")] AssertFocused,
    [JsonStringEnumMemberName("wait_unfocused")] WaitUnfocused,
    [JsonStringEnumMemberName("assert_unfocused")] AssertUnfocused,
    [JsonStringEnumMemberName("assert_enabled")] AssertEnabled,
    [JsonStringEnumMemberName("assert_disabled")] AssertDisabled,
    [JsonStringEnumMemberName("set_selection")] SetSelection,
    [JsonStringEnumMemberName("assert_selection")] AssertSelection,
    [JsonStringEnumMemberName("wait_selection")] WaitSelection,
    [JsonStringEnumMemberName("assert_child_count")] AssertChildCount,
    [JsonStringEnumMemberName("wait_child_count")] WaitChildCount,
    [JsonStringEnumMemberName("assert_text")] AssertText,
    [JsonStringEnumMemberName("wait_text")] WaitText,
    [JsonStringEnumMemberName("assert_automation_id")] AssertAutomationId,
    [JsonStringEnumMemberName("wait_automation_id")] WaitAutomationId,
    [JsonStringEnumMemberName("assert_node_kind")] AssertNodeKind,
    [JsonStringEnumMemberName("wait_node_kind")] WaitNodeKind,
    [JsonStringEnumMemberName("assert_action_kind")] AssertActionKind,
    [JsonStringEnumMemberName("wait_action_kind")] WaitActionKind,
    [JsonStringEnumMemberName("assert_action_label")] AssertActionLabel,
    [JsonStringEnumMemberName("wait_action_label")] WaitActionLabel,
    [JsonStringEnumMemberName("assert_action_available")] AssertActionAvailable,
    [JsonStringEnumMemberName("wait_action_available")] WaitActionAvailable,
    [JsonStringEnumMemberName("assert_action_unavailable_reason")] AssertActionUnavailableReason,
    [JsonStringEnumMemberName("wait_action_unavailable_reason")] WaitActionUnavailableReason,
    [JsonStringEnumMemberName("submit_form")] SubmitForm,
    [JsonStringEnumMemberName("cancel_form")] CancelForm,
    [JsonStringEnumMemberName("assert_form_field")] AssertFormField,
    [JsonStringEnumMemberName("assert_form_field_input_kind")] AssertFormFieldInputKind,
    [JsonStringEnumMemberName("assert_form_field_required")] AssertFormFieldRequired,
    [JsonStringEnumMemberName("assert_form_field_max_length")] AssertFormFieldMaxLength,
    [JsonStringEnumMemberName("assert_form_field_placeholder")] AssertFormFieldPlaceholder,
    [JsonStringEnumMemberName("wait_form_field")] WaitFormField,
    [JsonStringEnumMemberName("wait_form_field_input_kind")] WaitFormFieldInputKind,
    [JsonStringEnumMemberName("wait_form_field_required")] WaitFormFieldRequired,
    [JsonStringEnumMemberName("wait_form_field_max_length")] WaitFormFieldMaxLength,
    [JsonStringEnumMemberName("wait_form_field_placeholder")] WaitFormFieldPlaceholder,
    [JsonStringEnumMemberName("set_form_value")] SetFormValue,
    [JsonStringEnumMemberName("assert_form_value")] AssertFormValue,
    [JsonStringEnumMemberName("wait_form_value")] WaitFormValue,
    [JsonStringEnumMemberName("assert_accessible_name")] AssertAccessibleName,
    [JsonStringEnumMemberName("wait_accessible_name")] WaitAccessibleName,
    [JsonStringEnumMemberName("assert_accessible_description")] AssertAccessibleDescription,
    [JsonStringEnumMemberName("wait_accessible_description")] WaitAccessibleDescription,
}

[JsonConverter(typeof(JsonStringEnumConverter<UiFocusNavigationDirection>))]
public enum UiFocusNavigationDirection
{
    [JsonStringEnumMemberName("next")] Next,
    [JsonStringEnumMemberName("previous")] Previous,
    [JsonStringEnumMemberName("first")] First,
    [JsonStringEnumMemberName("last")] Last,
}

[JsonConverter(typeof(JsonStringEnumConverter<UiSelectionState>))]
public enum UiSelectionState
{
    [JsonStringEnumMemberName("selected")] Selected,
    [JsonStringEnumMemberName("unselected")] Unselected,
}

public enum UiPresentationValidation
{
    Valid,
    UnknownTarget,
    UnfocusableTarget,
    TextlessTarget,
    DescriptionlessTarget,
    FormlessTarget,
    UnknownFormField,
    SelectionlessTarget,
    InvalidExpectedText,
    InvalidExpectedKind,
    InvalidExpectedActionKind,
    InvalidExpectedInputKind,
    InvalidExpectedRequired,
    InvalidExpectedMaxLength,
    InvalidFormValue,
    InvalidNavigationDirection,
    InvalidSelectionState,
    InvalidExpectedChildCount,
    InvalidTimeout,
}

[JsonConverter(typeof(UiAdapterBindingKindJsonConverter))]
public enum UiAdapterBindingKind
{
    [JsonStringEnumMemberName("developer_owned_adapter")] DeveloperOwnedAdapter,
    [JsonStringEnumMemberName("generated_framework_binding")] GeneratedFrameworkBinding,
}

[JsonConverter(typeof(UiPresentationAtomJsonConverter))]
public enum UiPresentationAtom
{
    [JsonStringEnumMemberName("activate")] Activate,
    [JsonStringEnumMemberName("focus")] Focus,
    [JsonStringEnumMemberName("navigate_focus")] NavigateFocus,
    [JsonStringEnumMemberName("scroll_into_view")] ScrollIntoView,
    [JsonStringEnumMemberName("assert_visible")] AssertVisible,
    [JsonStringEnumMemberName("assert_hidden")] AssertHidden,
    [JsonStringEnumMemberName("wait_hidden")] WaitHidden,
    [JsonStringEnumMemberName("assert_realized")] AssertRealized,
    [JsonStringEnumMemberName("wait_realized")] WaitRealized,
    [JsonStringEnumMemberName("wait_visible")] WaitVisible,
    [JsonStringEnumMemberName("wait_enabled")] WaitEnabled,
    [JsonStringEnumMemberName("wait_focused")] WaitFocused,
    [JsonStringEnumMemberName("assert_focused")] AssertFocused,
    [JsonStringEnumMemberName("wait_unfocused")] WaitUnfocused,
    [JsonStringEnumMemberName("assert_unfocused")] AssertUnfocused,
    [JsonStringEnumMemberName("assert_enabled")] AssertEnabled,
    [JsonStringEnumMemberName("assert_disabled")] AssertDisabled,
    [JsonStringEnumMemberName("wait_disabled")] WaitDisabled,
    [JsonStringEnumMemberName("open_window")] OpenWindow,
    [JsonStringEnumMemberName("close_window")] CloseWindow,
    [JsonStringEnumMemberName("assert_window_open")] AssertWindowOpen,
    [JsonStringEnumMemberName("wait_window_open")] WaitWindowOpen,
    [JsonStringEnumMemberName("assert_window_closed")] AssertWindowClosed,
    [JsonStringEnumMemberName("wait_window_closed")] WaitWindowClosed,
    [JsonStringEnumMemberName("set_selection")] SetSelection,
    [JsonStringEnumMemberName("assert_selection")] AssertSelection,
    [JsonStringEnumMemberName("wait_selection")] WaitSelection,
    [JsonStringEnumMemberName("assert_child_count")] AssertChildCount,
    [JsonStringEnumMemberName("wait_child_count")] WaitChildCount,
    [JsonStringEnumMemberName("assert_text")] AssertText,
    [JsonStringEnumMemberName("wait_text")] WaitText,
    [JsonStringEnumMemberName("assert_automation_id")] AssertAutomationId,
    [JsonStringEnumMemberName("wait_automation_id")] WaitAutomationId,
    [JsonStringEnumMemberName("assert_node_kind")] AssertNodeKind,
    [JsonStringEnumMemberName("wait_node_kind")] WaitNodeKind,
    [JsonStringEnumMemberName("assert_action_kind")] AssertActionKind,
    [JsonStringEnumMemberName("wait_action_kind")] WaitActionKind,
    [JsonStringEnumMemberName("assert_action_label")] AssertActionLabel,
    [JsonStringEnumMemberName("wait_action_label")] WaitActionLabel,
    [JsonStringEnumMemberName("assert_action_available")] AssertActionAvailable,
    [JsonStringEnumMemberName("wait_action_available")] WaitActionAvailable,
    [JsonStringEnumMemberName("assert_action_unavailable_reason")] AssertActionUnavailableReason,
    [JsonStringEnumMemberName("wait_action_unavailable_reason")] WaitActionUnavailableReason,
    [JsonStringEnumMemberName("submit_form")] SubmitForm,
    [JsonStringEnumMemberName("cancel_form")] CancelForm,
    [JsonStringEnumMemberName("assert_form_field")] AssertFormField,
    [JsonStringEnumMemberName("wait_form_field")] WaitFormField,
    [JsonStringEnumMemberName("assert_form_field_input_kind")] AssertFormFieldInputKind,
    [JsonStringEnumMemberName("wait_form_field_input_kind")] WaitFormFieldInputKind,
    [JsonStringEnumMemberName("assert_form_field_required")] AssertFormFieldRequired,
    [JsonStringEnumMemberName("wait_form_field_required")] WaitFormFieldRequired,
    [JsonStringEnumMemberName("assert_form_field_max_length")] AssertFormFieldMaxLength,
    [JsonStringEnumMemberName("wait_form_field_max_length")] WaitFormFieldMaxLength,
    [JsonStringEnumMemberName("assert_form_field_placeholder")] AssertFormFieldPlaceholder,
    [JsonStringEnumMemberName("wait_form_field_placeholder")] WaitFormFieldPlaceholder,
    [JsonStringEnumMemberName("set_form_value")] SetFormValue,
    [JsonStringEnumMemberName("assert_form_value")] AssertFormValue,
    [JsonStringEnumMemberName("wait_form_value")] WaitFormValue,
    [JsonStringEnumMemberName("assert_accessible_name")] AssertAccessibleName,
    [JsonStringEnumMemberName("wait_accessible_name")] WaitAccessibleName,
    [JsonStringEnumMemberName("assert_accessible_description")] AssertAccessibleDescription,
    [JsonStringEnumMemberName("wait_accessible_description")] WaitAccessibleDescription,
}

[JsonConverter(typeof(UiPresentationAtomFamilyJsonConverter))]
public enum UiPresentationAtomFamily
{
    [JsonStringEnumMemberName("interaction")] Interaction,
    [JsonStringEnumMemberName("focus")] Focus,
    [JsonStringEnumMemberName("viewport")] Viewport,
    [JsonStringEnumMemberName("visibility")] Visibility,
    [JsonStringEnumMemberName("realization")] Realization,
    [JsonStringEnumMemberName("enabled_state")] EnabledState,
    [JsonStringEnumMemberName("window")] Window,
    [JsonStringEnumMemberName("selection")] Selection,
    [JsonStringEnumMemberName("structure")] Structure,
    [JsonStringEnumMemberName("text")] Text,
    [JsonStringEnumMemberName("node_metadata")] NodeMetadata,
    [JsonStringEnumMemberName("action_metadata")] ActionMetadata,
    [JsonStringEnumMemberName("form_lifecycle")] FormLifecycle,
    [JsonStringEnumMemberName("form_metadata")] FormMetadata,
    [JsonStringEnumMemberName("form_value")] FormValue,
    [JsonStringEnumMemberName("accessibility")] Accessibility,
}

[JsonConverter(typeof(UiPresentationAtomEffectJsonConverter))]
public enum UiPresentationAtomEffect
{
    [JsonStringEnumMemberName("mutation")] Mutation,
    [JsonStringEnumMemberName("assertion")] Assertion,
    [JsonStringEnumMemberName("wait")] Wait,
}

public enum UiAdapterManifestValidation
{
    Valid,
    UnsupportedManifestSchema,
    UnsupportedUiSchema,
    InvalidAdapterId,
    InvalidFramework,
    InvalidBindingKind,
    MissingDocumentSchema,
    MissingEventSchema,
    MissingPatchSchema,
    MissingPresentationAtom,
    InvalidPresentationAtom,
    MissingPresentationAtomProfile,
    DuplicatePresentationAtomProfile,
    InvalidPresentationAtomProfile,
    ProfileWithoutPresentationAtom,
    DuplicatePresentationAtom,
}

public sealed class UiAdapterBindingKindJsonConverter : JsonConverter<UiAdapterBindingKind>
{
    public override UiAdapterBindingKind Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        if (reader.TokenType != JsonTokenType.String)
        {
            throw new JsonException("adapter binding kind must be a string");
        }
        return reader.GetString() switch
        {
            "developer_owned_adapter" => UiAdapterBindingKind.DeveloperOwnedAdapter,
            "generated_framework_binding" => UiAdapterBindingKind.GeneratedFrameworkBinding,
            _ => throw new JsonException("unknown adapter binding kind"),
        };
    }

    public override void Write(
        Utf8JsonWriter writer,
        UiAdapterBindingKind value,
        JsonSerializerOptions options)
    {
        writer.WriteStringValue(value switch
        {
            UiAdapterBindingKind.DeveloperOwnedAdapter => "developer_owned_adapter",
            UiAdapterBindingKind.GeneratedFrameworkBinding => "generated_framework_binding",
            _ => throw new JsonException("unknown adapter binding kind"),
        });
    }
}

public sealed class UiPresentationAtomJsonConverter : JsonConverter<UiPresentationAtom>
{
    public override UiPresentationAtom Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        if (reader.TokenType != JsonTokenType.String)
        {
            throw new JsonException("presentation atom must be a string");
        }
        return reader.GetString() switch
        {
            "activate" => UiPresentationAtom.Activate,
            "focus" => UiPresentationAtom.Focus,
            "navigate_focus" => UiPresentationAtom.NavigateFocus,
            "scroll_into_view" => UiPresentationAtom.ScrollIntoView,
            "assert_visible" => UiPresentationAtom.AssertVisible,
            "assert_hidden" => UiPresentationAtom.AssertHidden,
            "wait_hidden" => UiPresentationAtom.WaitHidden,
            "assert_realized" => UiPresentationAtom.AssertRealized,
            "wait_realized" => UiPresentationAtom.WaitRealized,
            "wait_visible" => UiPresentationAtom.WaitVisible,
            "wait_enabled" => UiPresentationAtom.WaitEnabled,
            "wait_focused" => UiPresentationAtom.WaitFocused,
            "assert_focused" => UiPresentationAtom.AssertFocused,
            "wait_unfocused" => UiPresentationAtom.WaitUnfocused,
            "assert_unfocused" => UiPresentationAtom.AssertUnfocused,
            "assert_enabled" => UiPresentationAtom.AssertEnabled,
            "assert_disabled" => UiPresentationAtom.AssertDisabled,
            "wait_disabled" => UiPresentationAtom.WaitDisabled,
            "open_window" => UiPresentationAtom.OpenWindow,
            "close_window" => UiPresentationAtom.CloseWindow,
            "assert_window_open" => UiPresentationAtom.AssertWindowOpen,
            "wait_window_open" => UiPresentationAtom.WaitWindowOpen,
            "assert_window_closed" => UiPresentationAtom.AssertWindowClosed,
            "wait_window_closed" => UiPresentationAtom.WaitWindowClosed,
            "set_selection" => UiPresentationAtom.SetSelection,
            "assert_selection" => UiPresentationAtom.AssertSelection,
            "wait_selection" => UiPresentationAtom.WaitSelection,
            "assert_child_count" => UiPresentationAtom.AssertChildCount,
            "wait_child_count" => UiPresentationAtom.WaitChildCount,
            "assert_text" => UiPresentationAtom.AssertText,
            "wait_text" => UiPresentationAtom.WaitText,
            "assert_automation_id" => UiPresentationAtom.AssertAutomationId,
            "wait_automation_id" => UiPresentationAtom.WaitAutomationId,
            "assert_node_kind" => UiPresentationAtom.AssertNodeKind,
            "wait_node_kind" => UiPresentationAtom.WaitNodeKind,
            "assert_action_kind" => UiPresentationAtom.AssertActionKind,
            "wait_action_kind" => UiPresentationAtom.WaitActionKind,
            "assert_action_label" => UiPresentationAtom.AssertActionLabel,
            "wait_action_label" => UiPresentationAtom.WaitActionLabel,
            "assert_action_available" => UiPresentationAtom.AssertActionAvailable,
            "wait_action_available" => UiPresentationAtom.WaitActionAvailable,
            "assert_action_unavailable_reason" => UiPresentationAtom.AssertActionUnavailableReason,
            "wait_action_unavailable_reason" => UiPresentationAtom.WaitActionUnavailableReason,
            "submit_form" => UiPresentationAtom.SubmitForm,
            "cancel_form" => UiPresentationAtom.CancelForm,
            "assert_form_field" => UiPresentationAtom.AssertFormField,
            "wait_form_field" => UiPresentationAtom.WaitFormField,
            "assert_form_field_input_kind" => UiPresentationAtom.AssertFormFieldInputKind,
            "wait_form_field_input_kind" => UiPresentationAtom.WaitFormFieldInputKind,
            "assert_form_field_required" => UiPresentationAtom.AssertFormFieldRequired,
            "wait_form_field_required" => UiPresentationAtom.WaitFormFieldRequired,
            "assert_form_field_max_length" => UiPresentationAtom.AssertFormFieldMaxLength,
            "wait_form_field_max_length" => UiPresentationAtom.WaitFormFieldMaxLength,
            "assert_form_field_placeholder" => UiPresentationAtom.AssertFormFieldPlaceholder,
            "wait_form_field_placeholder" => UiPresentationAtom.WaitFormFieldPlaceholder,
            "set_form_value" => UiPresentationAtom.SetFormValue,
            "assert_form_value" => UiPresentationAtom.AssertFormValue,
            "wait_form_value" => UiPresentationAtom.WaitFormValue,
            "assert_accessible_name" => UiPresentationAtom.AssertAccessibleName,
            "wait_accessible_name" => UiPresentationAtom.WaitAccessibleName,
            "assert_accessible_description" => UiPresentationAtom.AssertAccessibleDescription,
            "wait_accessible_description" => UiPresentationAtom.WaitAccessibleDescription,
            _ => throw new JsonException("unknown presentation atom"),
        };
    }

    public override void Write(
        Utf8JsonWriter writer,
        UiPresentationAtom value,
        JsonSerializerOptions options)
    {
        writer.WriteStringValue(value switch
        {
            UiPresentationAtom.Activate => "activate",
            UiPresentationAtom.Focus => "focus",
            UiPresentationAtom.NavigateFocus => "navigate_focus",
            UiPresentationAtom.ScrollIntoView => "scroll_into_view",
            UiPresentationAtom.AssertVisible => "assert_visible",
            UiPresentationAtom.AssertHidden => "assert_hidden",
            UiPresentationAtom.WaitHidden => "wait_hidden",
            UiPresentationAtom.AssertRealized => "assert_realized",
            UiPresentationAtom.WaitRealized => "wait_realized",
            UiPresentationAtom.WaitVisible => "wait_visible",
            UiPresentationAtom.WaitEnabled => "wait_enabled",
            UiPresentationAtom.WaitFocused => "wait_focused",
            UiPresentationAtom.AssertFocused => "assert_focused",
            UiPresentationAtom.WaitUnfocused => "wait_unfocused",
            UiPresentationAtom.AssertUnfocused => "assert_unfocused",
            UiPresentationAtom.AssertEnabled => "assert_enabled",
            UiPresentationAtom.AssertDisabled => "assert_disabled",
            UiPresentationAtom.WaitDisabled => "wait_disabled",
            UiPresentationAtom.OpenWindow => "open_window",
            UiPresentationAtom.CloseWindow => "close_window",
            UiPresentationAtom.AssertWindowOpen => "assert_window_open",
            UiPresentationAtom.WaitWindowOpen => "wait_window_open",
            UiPresentationAtom.AssertWindowClosed => "assert_window_closed",
            UiPresentationAtom.WaitWindowClosed => "wait_window_closed",
            UiPresentationAtom.SetSelection => "set_selection",
            UiPresentationAtom.AssertSelection => "assert_selection",
            UiPresentationAtom.WaitSelection => "wait_selection",
            UiPresentationAtom.AssertChildCount => "assert_child_count",
            UiPresentationAtom.WaitChildCount => "wait_child_count",
            UiPresentationAtom.AssertText => "assert_text",
            UiPresentationAtom.WaitText => "wait_text",
            UiPresentationAtom.AssertAutomationId => "assert_automation_id",
            UiPresentationAtom.WaitAutomationId => "wait_automation_id",
            UiPresentationAtom.AssertNodeKind => "assert_node_kind",
            UiPresentationAtom.WaitNodeKind => "wait_node_kind",
            UiPresentationAtom.AssertActionKind => "assert_action_kind",
            UiPresentationAtom.WaitActionKind => "wait_action_kind",
            UiPresentationAtom.AssertActionLabel => "assert_action_label",
            UiPresentationAtom.WaitActionLabel => "wait_action_label",
            UiPresentationAtom.AssertActionAvailable => "assert_action_available",
            UiPresentationAtom.WaitActionAvailable => "wait_action_available",
            UiPresentationAtom.AssertActionUnavailableReason => "assert_action_unavailable_reason",
            UiPresentationAtom.WaitActionUnavailableReason => "wait_action_unavailable_reason",
            UiPresentationAtom.SubmitForm => "submit_form",
            UiPresentationAtom.CancelForm => "cancel_form",
            UiPresentationAtom.AssertFormField => "assert_form_field",
            UiPresentationAtom.WaitFormField => "wait_form_field",
            UiPresentationAtom.AssertFormFieldInputKind => "assert_form_field_input_kind",
            UiPresentationAtom.WaitFormFieldInputKind => "wait_form_field_input_kind",
            UiPresentationAtom.AssertFormFieldRequired => "assert_form_field_required",
            UiPresentationAtom.WaitFormFieldRequired => "wait_form_field_required",
            UiPresentationAtom.AssertFormFieldMaxLength => "assert_form_field_max_length",
            UiPresentationAtom.WaitFormFieldMaxLength => "wait_form_field_max_length",
            UiPresentationAtom.AssertFormFieldPlaceholder => "assert_form_field_placeholder",
            UiPresentationAtom.WaitFormFieldPlaceholder => "wait_form_field_placeholder",
            UiPresentationAtom.SetFormValue => "set_form_value",
            UiPresentationAtom.AssertFormValue => "assert_form_value",
            UiPresentationAtom.WaitFormValue => "wait_form_value",
            UiPresentationAtom.AssertAccessibleName => "assert_accessible_name",
            UiPresentationAtom.WaitAccessibleName => "wait_accessible_name",
            UiPresentationAtom.AssertAccessibleDescription => "assert_accessible_description",
            UiPresentationAtom.WaitAccessibleDescription => "wait_accessible_description",
            _ => throw new JsonException("unknown presentation atom"),
        });
    }
}

public sealed class UiPresentationAtomFamilyJsonConverter : JsonConverter<UiPresentationAtomFamily>
{
    public override UiPresentationAtomFamily Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        if (reader.TokenType != JsonTokenType.String)
        {
            throw new JsonException("presentation atom family must be a string");
        }
        return reader.GetString() switch
        {
            "interaction" => UiPresentationAtomFamily.Interaction,
            "focus" => UiPresentationAtomFamily.Focus,
            "viewport" => UiPresentationAtomFamily.Viewport,
            "visibility" => UiPresentationAtomFamily.Visibility,
            "realization" => UiPresentationAtomFamily.Realization,
            "enabled_state" => UiPresentationAtomFamily.EnabledState,
            "window" => UiPresentationAtomFamily.Window,
            "selection" => UiPresentationAtomFamily.Selection,
            "structure" => UiPresentationAtomFamily.Structure,
            "text" => UiPresentationAtomFamily.Text,
            "node_metadata" => UiPresentationAtomFamily.NodeMetadata,
            "action_metadata" => UiPresentationAtomFamily.ActionMetadata,
            "form_lifecycle" => UiPresentationAtomFamily.FormLifecycle,
            "form_metadata" => UiPresentationAtomFamily.FormMetadata,
            "form_value" => UiPresentationAtomFamily.FormValue,
            "accessibility" => UiPresentationAtomFamily.Accessibility,
            _ => throw new JsonException("unknown presentation atom family"),
        };
    }

    public override void Write(
        Utf8JsonWriter writer,
        UiPresentationAtomFamily value,
        JsonSerializerOptions options)
    {
        writer.WriteStringValue(value switch
        {
            UiPresentationAtomFamily.Interaction => "interaction",
            UiPresentationAtomFamily.Focus => "focus",
            UiPresentationAtomFamily.Viewport => "viewport",
            UiPresentationAtomFamily.Visibility => "visibility",
            UiPresentationAtomFamily.Realization => "realization",
            UiPresentationAtomFamily.EnabledState => "enabled_state",
            UiPresentationAtomFamily.Window => "window",
            UiPresentationAtomFamily.Selection => "selection",
            UiPresentationAtomFamily.Structure => "structure",
            UiPresentationAtomFamily.Text => "text",
            UiPresentationAtomFamily.NodeMetadata => "node_metadata",
            UiPresentationAtomFamily.ActionMetadata => "action_metadata",
            UiPresentationAtomFamily.FormLifecycle => "form_lifecycle",
            UiPresentationAtomFamily.FormMetadata => "form_metadata",
            UiPresentationAtomFamily.FormValue => "form_value",
            UiPresentationAtomFamily.Accessibility => "accessibility",
            _ => throw new JsonException("unknown presentation atom family"),
        });
    }
}

public sealed class UiPresentationAtomEffectJsonConverter : JsonConverter<UiPresentationAtomEffect>
{
    public override UiPresentationAtomEffect Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        if (reader.TokenType != JsonTokenType.String)
        {
            throw new JsonException("presentation atom effect must be a string");
        }
        return reader.GetString() switch
        {
            "mutation" => UiPresentationAtomEffect.Mutation,
            "assertion" => UiPresentationAtomEffect.Assertion,
            "wait" => UiPresentationAtomEffect.Wait,
            _ => throw new JsonException("unknown presentation atom effect"),
        };
    }

    public override void Write(
        Utf8JsonWriter writer,
        UiPresentationAtomEffect value,
        JsonSerializerOptions options)
    {
        writer.WriteStringValue(value switch
        {
            UiPresentationAtomEffect.Mutation => "mutation",
            UiPresentationAtomEffect.Assertion => "assertion",
            UiPresentationAtomEffect.Wait => "wait",
            _ => throw new JsonException("unknown presentation atom effect"),
        });
    }
}

[JsonConverter(typeof(JsonStringEnumConverter<PatchKind>))]
public enum PatchKind
{
    [JsonStringEnumMemberName("remove")] Remove,
    [JsonStringEnumMemberName("insert")] Insert,
    [JsonStringEnumMemberName("move")] Move,
    [JsonStringEnumMemberName("update")] Update,
}

[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(RendererFixture))]
[JsonSerializable(typeof(UiDocument))]
[JsonSerializable(typeof(UiEvent))]
[JsonSerializable(typeof(UiPresentationOperation))]
[JsonSerializable(typeof(UiAdapterManifest))]
[JsonSerializable(typeof(UiPresentationAtomProfile))]
public partial class RendererJsonContext : JsonSerializerContext;
