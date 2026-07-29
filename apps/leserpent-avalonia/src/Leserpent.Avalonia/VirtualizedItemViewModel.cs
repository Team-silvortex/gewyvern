using Avalonia.Controls;

namespace Leserpent.Avalonia;

public sealed class VirtualizedItemViewModel(
    string nodeId,
    UiSelectionState? selectionState,
    Func<Control> contentFactory)
{
    private Control? content;

    public string NodeId { get; } = nodeId;
    public UiSelectionState? SelectionState { get; } = selectionState;
    public Control Content => content ??= contentFactory();
    public bool IsRealized => content is not null;

    public bool TryGetContent(out Control? realized)
    {
        realized = content;
        return realized is not null;
    }
}
