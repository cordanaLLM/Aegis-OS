// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Converts an [`accesskit::TreeUpdate`] (0.25.1) from the M16 model crate into
//! a [`gpui::accesskit::TreeUpdate`] (0.24.1) for gpui (M28 criterion 1).

use thiserror::Error;

/// Scalar upper bound on nodes in one tree update (HISS-02).
pub const MAX_NODES: usize = 65536;

/// Scalar upper bound on children of one node (HISS-02).
pub const MAX_CHILDREN: usize = 16384;

/// Why the tree update could not be converted for gpui.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ConvertError {
    /// The update carries no tree information, so it names no root.
    #[error("the update carries no tree information and names no root")]
    MissingTreeInfo,

    /// The tree update specifies an unsupported non-root tree ID.
    #[error("unsupported non-root tree id {0:?}")]
    NonRootTreeId(accesskit::TreeId),

    /// A node carries a property unsupported in gpui's AccessKit version.
    #[error("node {node_id:?} sets unsupported property: {property}")]
    UnsupportedProperty {
        /// The node ID that holds the unsupported property.
        node_id: accesskit::NodeId,
        /// The name of the unsupported property.
        property: &'static str,
    },

    /// A node specifies an unsupported role.
    #[error("node {node_id:?} has unsupported role: {role:?}")]
    UnsupportedRole {
        /// The node ID that holds the unsupported role.
        node_id: accesskit::NodeId,
        /// The unsupported role.
        role: accesskit::Role,
    },

    /// The update carries more nodes than the scalar bound admits.
    #[error("update carries {0} nodes, exceeding scalar bound of {MAX_NODES}")]
    TooManyNodes(usize),

    /// A node carries more children than the scalar bound admits.
    #[error("node {node_id:?} carries {count} children, exceeding scalar bound of {MAX_CHILDREN}")]
    TooManyChildren {
        /// The node ID that holds too many children.
        node_id: accesskit::NodeId,
        /// The number of children.
        count: usize,
    },
}

// Role enum variants common to accesskit 0.24.1 and accesskit 0.25.1 (182 total).
// Variants present only in accesskit 0.25.1: none (diff between 0.24.1 and 0.25.1 is empty: []).
macro_rules! role_pairs {
    ($( $r:ident ),* $(,)?) => {
        /// Compile-checked role pairs between accesskit 0.25.1 and gpui's 0.24.1.
        #[doc(hidden)]
        pub const ROLE_PAIRS: [(accesskit::Role, gpui::accesskit::Role); [$(stringify!($r)),*].len()] = [
            $((accesskit::Role::$r, gpui::accesskit::Role::$r)),*
        ];
    };
}

role_pairs!(
    Unknown,
    TextRun,
    Cell,
    Label,
    Image,
    Link,
    Row,
    ListItem,
    ListMarker,
    TreeItem,
    ListBoxOption,
    MenuItem,
    MenuListOption,
    Paragraph,
    GenericContainer,
    CheckBox,
    RadioButton,
    TextInput,
    Button,
    DefaultButton,
    Pane,
    RowHeader,
    ColumnHeader,
    RowGroup,
    List,
    Table,
    LayoutTableCell,
    LayoutTableRow,
    LayoutTable,
    Switch,
    Menu,
    MultilineTextInput,
    SearchInput,
    DateInput,
    DateTimeInput,
    WeekInput,
    MonthInput,
    TimeInput,
    EmailInput,
    NumberInput,
    PasswordInput,
    PhoneNumberInput,
    UrlInput,
    Abbr,
    Alert,
    AlertDialog,
    Application,
    Article,
    Audio,
    Banner,
    Blockquote,
    Canvas,
    Caption,
    Caret,
    Code,
    ColorWell,
    ComboBox,
    EditableComboBox,
    Complementary,
    Comment,
    ContentDeletion,
    ContentInsertion,
    ContentInfo,
    Definition,
    DescriptionList,
    Details,
    Dialog,
    DisclosureTriangle,
    Document,
    EmbeddedObject,
    Emphasis,
    Feed,
    FigureCaption,
    Figure,
    Footer,
    Form,
    Grid,
    GridCell,
    Group,
    Header,
    Heading,
    Iframe,
    IframePresentational,
    ImeCandidate,
    Keyboard,
    Legend,
    LineBreak,
    ListBox,
    Log,
    Main,
    Mark,
    Marquee,
    Math,
    MenuBar,
    MenuItemCheckBox,
    MenuItemRadio,
    MenuListPopup,
    Meter,
    Navigation,
    Note,
    PluginObject,
    ProgressIndicator,
    RadioGroup,
    Region,
    RootWebArea,
    Ruby,
    RubyAnnotation,
    ScrollBar,
    ScrollView,
    Search,
    Section,
    SectionFooter,
    SectionHeader,
    Slider,
    SpinButton,
    Splitter,
    Status,
    Strong,
    Suggestion,
    SvgRoot,
    Tab,
    TabList,
    TabPanel,
    Term,
    Time,
    Timer,
    TitleBar,
    Toolbar,
    Tooltip,
    Tree,
    TreeGrid,
    Video,
    WebView,
    Window,
    PdfActionableHighlight,
    PdfRoot,
    GraphicsDocument,
    GraphicsObject,
    GraphicsSymbol,
    DocAbstract,
    DocAcknowledgements,
    DocAfterword,
    DocAppendix,
    DocBackLink,
    DocBiblioEntry,
    DocBibliography,
    DocBiblioRef,
    DocChapter,
    DocColophon,
    DocConclusion,
    DocCover,
    DocCredit,
    DocCredits,
    DocDedication,
    DocEndnote,
    DocEndnotes,
    DocEpigraph,
    DocEpilogue,
    DocErrata,
    DocExample,
    DocFootnote,
    DocForeword,
    DocGlossary,
    DocGlossRef,
    DocIndex,
    DocIntroduction,
    DocNoteRef,
    DocNotice,
    DocPageBreak,
    DocPageFooter,
    DocPageHeader,
    DocPageList,
    DocPart,
    DocPreface,
    DocPrologue,
    DocPullquote,
    DocQna,
    DocSubtitle,
    DocTip,
    DocToc,
    ListGrid,
    Terminal,
);

// Action enum variants common to accesskit 0.24.1 and accesskit 0.25.1 (22 total).
// Variants present only in accesskit 0.25.1: none (diff between 0.24.1 and 0.25.1 is empty: []).
macro_rules! action_pairs {
    ($( $a:ident ),* $(,)?) => {
        /// Compile-checked action pairs between accesskit 0.25.1 and gpui's 0.24.1.
        const ACTIONS: [(accesskit::Action, gpui::accesskit::Action); [$(stringify!($a)),*].len()] = [
            $((accesskit::Action::$a, gpui::accesskit::Action::$a)),*
        ];
    };
}

action_pairs!(
    Click,
    Focus,
    Blur,
    Collapse,
    Expand,
    CustomAction,
    Decrement,
    Increment,
    HideTooltip,
    ShowTooltip,
    ReplaceSelectedText,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    ScrollUp,
    ScrollIntoView,
    ScrollToPoint,
    SetScrollOffset,
    SetTextSelection,
    SetSequentialFocusNavigationStartingPoint,
    SetValue,
    ShowContextMenu,
);

fn convert_role(
    role: accesskit::Role,
    node_id: accesskit::NodeId,
) -> Result<gpui::accesskit::Role, ConvertError> {
    for &(src, dst) in ROLE_PAIRS.iter().take(ROLE_PAIRS.len()) {
        if src == role {
            return Ok(dst);
        }
    }
    Err(ConvertError::UnsupportedRole { node_id, role })
}

fn convert_actions(source: &accesskit::Node, target: &mut gpui::accesskit::Node) {
    for &(src_action, tgt_action) in ACTIONS.iter().take(ACTIONS.len()) {
        if source.supports_action(src_action) {
            target.add_action(tgt_action);
        }
    }
}

fn convert_children(
    source: &accesskit::Node,
    target: &mut gpui::accesskit::Node,
    node_id: accesskit::NodeId,
) -> Result<(), ConvertError> {
    let children = source.children();
    if children.is_empty() {
        return Ok(());
    }
    if children.len() > MAX_CHILDREN {
        return Err(ConvertError::TooManyChildren {
            node_id,
            count: children.len(),
        });
    }
    let mut mapped = Vec::with_capacity(children.len());
    for &child in children.iter().take(MAX_CHILDREN) {
        mapped.push(gpui::accesskit::NodeId(child.0));
    }
    target.set_children(mapped);
    Ok(())
}

fn copy_node_properties(source: &accesskit::Node, target: &mut gpui::accesskit::Node) {
    if let Some(label) = source.label() {
        target.set_label(label);
    }
    if let Some(bounds) = source.bounds() {
        target.set_bounds(gpui::accesskit::Rect {
            x0: bounds.x0,
            y0: bounds.y0,
            x1: bounds.x1,
            y1: bounds.y1,
        });
    }
    if let Some(desc) = source.role_description() {
        target.set_role_description(desc);
    }
    if source.is_hidden() {
        target.set_hidden();
    }
}

fn rebuild_mapped_node(source: &accesskit::Node) -> accesskit::Node {
    let mut rebuilt = accesskit::Node::new(source.role());
    if let Some(label) = source.label() {
        rebuilt.set_label(label);
    }
    if let Some(bounds) = source.bounds() {
        rebuilt.set_bounds(bounds);
    }
    if let Some(desc) = source.role_description() {
        rebuilt.set_role_description(desc);
    }
    if source.is_hidden() {
        rebuilt.set_hidden();
    }
    for &(src_action, _) in ACTIONS.iter().take(ACTIONS.len()) {
        if source.supports_action(src_action) {
            rebuilt.add_action(src_action);
        }
    }
    let children = source.children();
    if !children.is_empty() {
        rebuilt.set_children(children.to_vec());
    }
    rebuilt
}

fn is_rebuilt_equal(source: &accesskit::Node) -> bool {
    let mut rebuilt = rebuild_mapped_node(source);
    if rebuilt == *source {
        return true;
    }
    if source.children().is_empty() {
        rebuilt.set_children(Vec::new());
        if rebuilt == *source {
            return true;
        }
    }
    false
}

fn convert_node(
    node_id: accesskit::NodeId,
    source: &accesskit::Node,
) -> Result<gpui::accesskit::Node, ConvertError> {
    if source.html_id().is_some() {
        return Err(ConvertError::UnsupportedProperty {
            node_id,
            property: "html_id",
        });
    }
    if let Some(tree_id) = source
        .tree_id()
        .filter(|&tid| tid != accesskit::TreeId::ROOT)
    {
        return Err(ConvertError::NonRootTreeId(tree_id));
    }
    let role = convert_role(source.role(), node_id)?;
    let mut target = gpui::accesskit::Node::new(role);
    copy_node_properties(source, &mut target);
    convert_actions(source, &mut target);
    convert_children(source, &mut target, node_id)?;

    if !is_rebuilt_equal(source) {
        return Err(ConvertError::UnsupportedProperty {
            node_id,
            property: "unmapped property",
        });
    }

    Ok(target)
}

/// Converts an [`accesskit::TreeUpdate`] (0.25.1) to [`gpui::accesskit::TreeUpdate`] (0.24.1).
///
/// Maps every field the M16 export sets: nodes, ids, role, label, bounds,
/// actions, children, hidden, focus, and tree root. Any unsupported property or
/// non-root tree ID returns a named [`ConvertError`].
///
/// # Errors
///
/// Returns [`ConvertError::NonRootTreeId`] if the update or any node contains a non-root tree ID.
/// Returns [`ConvertError::MissingTreeInfo`] if `update.tree` is `None`.
/// Returns [`ConvertError::TooManyNodes`] if node count exceeds [`MAX_NODES`].
/// Returns [`ConvertError::UnsupportedProperty`] if an unsupported field is set.
/// Returns [`ConvertError::UnsupportedRole`] if a role cannot be mapped.
/// Returns [`ConvertError::TooManyChildren`] if child count exceeds [`MAX_CHILDREN`].
pub fn to_gpui(
    update: &accesskit::TreeUpdate,
) -> Result<gpui::accesskit::TreeUpdate, ConvertError> {
    if update.tree_id != accesskit::TreeId::ROOT {
        return Err(ConvertError::NonRootTreeId(update.tree_id));
    }
    let tree_info = update.tree.as_ref().ok_or(ConvertError::MissingTreeInfo)?;
    if update.nodes.len() > MAX_NODES {
        return Err(ConvertError::TooManyNodes(update.nodes.len()));
    }
    let mut tree = gpui::accesskit::Tree::new(gpui::accesskit::NodeId(tree_info.root.0));
    tree.toolkit_name.clone_from(&tree_info.toolkit_name);
    tree.toolkit_version.clone_from(&tree_info.toolkit_version);

    let mut nodes = Vec::with_capacity(update.nodes.len());
    for (id, node) in update.nodes.iter().take(MAX_NODES) {
        let converted = convert_node(*id, node)?;
        nodes.push((gpui::accesskit::NodeId(id.0), converted));
    }

    Ok(gpui::accesskit::TreeUpdate {
        nodes,
        tree: Some(tree),
        tree_id: gpui::accesskit::TreeId::ROOT,
        focus: gpui::accesskit::NodeId(update.focus.0),
    })
}
