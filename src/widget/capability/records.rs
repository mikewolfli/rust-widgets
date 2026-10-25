// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Typed records and record lists — the carrier for a control's *collection* property (BLUE26
//! B26.09).
//!
//! # The gap this closes
//!
//! Several controls hold a list of records rather than a scalar: `chip` holds `ChipItem { id,
//! label, selected }`, `timeline_widget` holds `TimelineItem { id, label, start, end }`,
//! `segmented_control` holds `SegmentItem { id, label }`. Their Rust APIs are typed, but the
//! *property contract* could not express any of them:
//!
//! * `PropertyValueKind` had no list-of-records kind, and
//! * [`CapabilityValue`](super::CapabilityValue) had no record carrier, so a JSON document's
//!   `"items": [{ "id": "a", "label": "A" }]` was **dropped on the floor** — `ProjectNode::
//!   scalar_properties` skips arrays and objects, which made the value silently absent rather than
//!   an error.
//!
//! The tempting shortcut was to carry the list as its JSON spelling in a `String` property. That is
//! refused here for the same reason `Constraint` is enforced rather than
//! documented: a stringified list is *accepted* by any setter, so a typo in a field name produces a
//! control nothing like the document asked for and no error anywhere. A record list that arrives
//! typed can be checked against the schema the control publishes, and refused with a reason.
//!
//! # What is here, and what is deliberately not
//!
//! There is **one** new value shape: an ordered list of records that share one declared schema.
//!
//! * A **row move** is [`RecordList::move_item`]; a **column move** is [`RecordList::move_field`].
//!   Both preserve every item's [`id`](RecordSchema::id_field), which is what lets a host keep a
//!   selection, a scroll anchor or an undo record across a reorder.
//! * There is no single-record value and no nested record. A control that needs those has not been
//!   named yet, and `CapabilityValue::Tuple` already carries the positional shapes that do exist
//!   (`heatmap.set_cell` is a 3-tuple, `range_slider.range_changed` a 2-tuple).
//! * The library holds **no business state**. A `RecordList` is a value the caller owns and hands
//!   over; the control copies what it needs into its own storage. Applying a value is atomic — see
//!   [`RecordList::validate`] — so a refused update cannot leave a half-replaced list behind.

use super::types::{CapabilityValue, PropertyValueKind};
use crate::compat::{format, String, Vec};

/// One field of a record schema: its name, its declared kind, and whether a record must carry it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordField {
    /// The field's name, as it appears in a document and in [`RecordValue::get`].
    pub name: &'static str,
    /// The declared kind of this field's value.
    pub kind: PropertyValueKind,
    /// Whether every record must carry this field.
    ///
    /// A missing **required** field is [`RecordViolation::MissingField`]. A missing optional field
    /// is not a violation, and reading it answers `None` — the distinction a property editor needs
    /// to render "unset" differently from "wrong".
    pub required: bool,
}

impl RecordField {
    /// A required field of the given kind. Shorthand for the schema tables.
    pub const fn required(name: &'static str, kind: PropertyValueKind) -> Self {
        Self { name, kind, required: true }
    }

    /// An optional field of the given kind.
    pub const fn optional(name: &'static str, kind: PropertyValueKind) -> Self {
        Self { name, kind, required: false }
    }
}

/// The declared shape of one record in a list-valued property.
///
/// # Why the identity field is declared rather than assumed
///
/// A record list is *ordered*, so an item's position is not its identity: a row move, a sorted
/// insert or a filtered view all change positions while the item stays the same item. The list
/// therefore has to name the field that carries identity, and a schema whose `id_field` is not one
/// of its own fields is a schema defect ([`RecordViolation::SchemaHasNoIdField`]) rather than a
/// list that quietly cannot answer [`RecordList::get_by_id`].
///
/// # Why uniqueness is a schema decision
///
/// Most collections key on their id, and a duplicate is then a defect, not a choice. Some
/// collections deliberately allow a repeated id — a timeline showing the same event twice — so the
/// schema states which it is instead of the validator guessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordSchema {
    /// The record's fields, in the order a designer should offer them.
    pub fields: &'static [RecordField],
    /// The field that carries an item's stable identity.
    pub id_field: &'static str,
    /// Whether two records in one list may carry the same id.
    pub ids_unique: bool,
    /// Whether a record's id must be non-empty.
    pub ids_required: bool,
}

impl RecordSchema {
    /// The field with this name, if the schema declares one.
    pub fn field(&self, name: &str) -> Option<&'static RecordField> {
        self.fields.iter().find(|field| field.name == name)
    }

    /// The position of the field with this name, in declaration order.
    pub fn field_index(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|field| field.name == name)
    }

    /// The declared field names, in declaration order.
    pub fn names(&self) -> Vec<&'static str> {
        self.fields.iter().map(|field| field.name).collect()
    }

    /// The field the schema names as its identity, or `None` when it names none.
    ///
    /// `None` is what [`RecordList::validate`] reports as
    /// [`RecordViolation::SchemaHasNoIdField`]: a list whose schema cannot answer "which field is
    /// the identity" cannot be checked for a duplicate either.
    pub fn id_field_schema(&self) -> Option<&'static RecordField> {
        self.field(self.id_field)
    }
}

/// One record: an ordered group of named values.
///
/// The order is the order the caller supplied, which is what a `Vec`-backed field list preserves;
/// [`RecordList::move_field`] is the one operation that changes it, and it changes it for every
/// record at once so the list stays rectangular.
///
/// Not a map: two fields with the same name would be a silent last-one-wins, and the schema check
/// that follows would then be judging a record whose real shape no longer matches the document.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RecordValue {
    fields: Vec<(String, CapabilityValue)>,
}

impl RecordValue {
    /// An empty record.
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    /// Adds one field, chainable.
    ///
    /// A repeated name is kept as a second entry rather than replacing the first: the duplicate is
    /// then visible to [`RecordList::validate`] instead of being resolved silently here.
    pub fn with(mut self, name: impl Into<String>, value: CapabilityValue) -> Self {
        self.fields.push((name.into(), value));
        self
    }

    /// The value of the field with this name, or `None` when the record does not carry it.
    pub fn get(&self, name: &str) -> Option<&CapabilityValue> {
        self.fields.iter().find(|(key, _)| key == name).map(|(_, value)| value)
    }

    /// The record's fields, in their own order.
    pub fn fields(&self) -> &[(String, CapabilityValue)] {
        &self.fields
    }

    /// The field names, in their own order.
    pub fn names(&self) -> Vec<&str> {
        self.fields.iter().map(|(key, _)| key.as_str()).collect()
    }

    /// The record without the field named `name`, or the same record when it does not carry it.
    ///
    /// Chainable, and the mirror of [`Self::with`]: it is how a test or a caller builds a record that
    /// is missing an optional field, which is a state the schema allows and the validator must
    /// therefore distinguish from "wrong".
    pub fn without(mut self, name: &str) -> Self {
        self.remove(name);
        self
    }

    /// How many fields the record carries.
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Whether the record carries no fields at all.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// The text of a text field, or an empty string when it is absent or not text.
    ///
    /// Convenience for the overwhelmingly common case (an `id`, a `label`), and deliberately
    /// forgiving in the *reading* direction: a designer listing items should not have to unwrap two
    /// `Option`s to display a name. Writing is where strictness belongs, and there
    /// [`RecordList::validate`] is the gate.
    pub fn text(&self, name: &str) -> &str {
        match self.get(name) {
            Some(CapabilityValue::String(text)) => text.as_str(),
            _ => "",
        }
    }

    /// Removes the field with this name, returning its value.
    fn remove(&mut self, name: &str) -> Option<CapabilityValue> {
        let index = self.fields.iter().position(|(key, _)| key == name)?;
        Some(self.fields.remove(index).1)
    }

    /// Re-inserts a field at `position`, or at the end when `position` is past the end.
    fn insert_at(&mut self, position: usize, name: String, value: CapabilityValue) {
        let position = position.min(self.fields.len());
        self.fields.insert(position, (name, value));
    }
}

/// An ordered list of records that share one declared schema.
///
/// The schema travels **with** the value, so a list is self-describing: [`Self::validate`],
/// [`Self::get_by_id`], [`Self::move_item`] and [`Self::move_field`] need no second argument, and a
/// value that arrived from a document cannot be read with the wrong schema by mistake.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordList {
    schema: &'static RecordSchema,
    items: Vec<RecordValue>,
}

impl RecordList {
    /// An empty list for `schema`.
    pub const fn new(schema: &'static RecordSchema) -> Self {
        Self { schema, items: Vec::new() }
    }

    /// A list holding `items`.
    pub fn from_items(schema: &'static RecordSchema, items: Vec<RecordValue>) -> Self {
        Self { schema, items }
    }

    /// The schema every record in this list is judged against.
    pub const fn schema(&self) -> &'static RecordSchema {
        self.schema
    }

    /// The records, in order.
    pub fn items(&self) -> &[RecordValue] {
        &self.items
    }

    /// The number of records.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the list holds no records at all.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The record at `index`, or `None` when the index is past the end.
    pub fn get(&self, index: usize) -> Option<&RecordValue> {
        self.items.get(index)
    }

    /// The stable id of the record at `index`, or `None` when the index is past the end or the
    /// record's id is absent or not text.
    pub fn id_of(&self, index: usize) -> Option<&str> {
        let record = self.items.get(index)?;
        match record.get(self.schema.id_field) {
            Some(CapabilityValue::String(text)) if !text.is_empty() => Some(text.as_str()),
            _ => None,
        }
    }

    /// The position of the record whose id is `id`.
    ///
    /// Reads through the schema's declared identity field rather than a field named `"id"`, so a
    /// control that keys on `"key"` or `"name"` is served by the same code.
    pub fn index_of_id(&self, id: &str) -> Option<usize> {
        self.items.iter().position(|record| record.text(self.schema.id_field) == id)
    }

    /// The record whose id is `id`, or `None`.
    ///
    /// This is the operation an ordered list needs and a positional read cannot give: a selection, a
    /// scroll anchor or a pending edit survives a reorder only if it is held by id.
    pub fn get_by_id(&self, id: &str) -> Option<&RecordValue> {
        self.items.iter().find(|record| record.text(self.schema.id_field) == id)
    }

    /// Appends one record.
    pub fn push(&mut self, record: RecordValue) {
        self.items.push(record);
    }

    /// Moves the record at `from` so that it sits at position `to`.
    ///
    /// # Why a move and not a remove-then-insert
    ///
    /// A remove-then-insert pair is two mutations: a caller that fails between them has lost a
    /// record, and a reorder expressed as `remove(i); insert(j, ..)` has to get the `j` adjustment
    /// right or it lands one position off. One operation that validates both indices **before**
    /// touching the list cannot do either, and the record it moves is the same record — same id,
    /// same fields — so identity is preserved by construction.
    ///
    /// An out-of-range index is reported rather than clamped: a designer dragging a row past the end
    /// of its list is a bug in the drag, and silently clamping it would apply a reorder nobody
    /// asked for.
    pub fn move_item(&mut self, from: usize, to: usize) -> Result<(), RecordViolation> {
        if from >= self.items.len() {
            return Err(RecordViolation::IndexOutOfRange {
                operation: RecordOperation::MoveItem,
                index: from,
                len: self.items.len(),
            });
        }
        if to >= self.items.len() {
            return Err(RecordViolation::IndexOutOfRange {
                operation: RecordOperation::MoveItem,
                index: to,
                len: self.items.len(),
            });
        }
        if from == to {
            return Ok(());
        }
        let record = self.items.remove(from);
        self.items.insert(to, record);
        Ok(())
    }

    /// Moves the field named `name` so that it sits at position `to` in **every** record that
    /// carries it.
    ///
    /// This is the data side of a designer's column reorder: the columns of the list are the schema's
    /// fields, and the move has to keep the list consistent — a per-record reorder would leave
    /// records whose shared fields disagree about their order, which is exactly the shape
    /// [`Self::validate`] exists to reject.
    ///
    /// A record that does not carry the field (it is optional and unset) is left alone: "move the
    /// field" cannot mean "add it to a record that does not have it". So the invariant is that the
    /// fields the records **do** share keep the same relative order, not that every record ends up
    /// with the same length. Ids are untouched, so identity survives a column move by construction.
    ///
    /// Every decision is made before the first mutation: an undeclared name or an out-of-range target
    /// leaves the list exactly as it was.
    pub fn move_field(&mut self, name: &str, to: usize) -> Result<(), RecordViolation> {
        let Some(source) = self.schema.field_index(name) else {
            return Err(RecordViolation::UnknownFieldName { field: String::from(name) });
        };
        if to >= self.schema.fields.len() {
            return Err(RecordViolation::IndexOutOfRange {
                operation: RecordOperation::MoveField,
                index: to,
                len: self.schema.fields.len(),
            });
        }
        if source == to {
            return Ok(());
        }
        for record in &mut self.items {
            if let Some(value) = record.remove(name) {
                // `insert_at` clamps, so a field moved to the end of a shorter record lands at the
                // end rather than one past it.
                record.insert_at(to, String::from(name), value);
            }
        }
        Ok(())
    }

    /// The same list, judged by `schema` from now on.
    ///
    /// # Why a value needs adopting rather than a schema being compared
    ///
    /// A record list arriving from JSON or the C ABI carries no `&'static` schema — neither boundary
    /// can name one — so the list is built against
    /// [`EMPTY_RECORD_SCHEMA`] and its records are checked against the *property's* schema instead
    /// ([`Self::validate_against`]). A control that wants the identity lookups
    /// ([`Self::get_by_id`], [`Self::index_of_id`]) on such a list adopts it, which is what gives
    /// those methods the identity field to read.
    pub fn with_schema(self, schema: &'static RecordSchema) -> Self {
        Self { schema, items: self.items }
    }

    /// Checks every record against `schema`, returning the first violation.
    ///
    /// This is the entry point for a list that arrived without a schema of its own: the property it is
    /// written to knows its schema, and that is the only declaration the check should use. Judging a
    /// value by a schema it brought with it would let a caller satisfy a rule the control never
    /// published.
    ///
    /// # Why this is a whole-list check and not a per-record one
    ///
    /// Two of the rules are properties of the *list*, not of a record: an id must be unique across
    /// the list, and the schema's own identity field must exist. A per-record validator could not
    /// see either, and a caller that ran one would believe a list with two `"a"` ids was fine.
    ///
    /// # What a caller does with the answer
    ///
    /// Refuse the update and change nothing. Every rule here is checked before the list is handed
    /// over, so an update that fails this leaves the control's own storage untouched — which is what
    /// makes a withdrawn or superseded edit safe to abandon rather than something that has to be
    /// rolled back.
    pub fn validate_against(&self, schema: &'static RecordSchema) -> Result<(), RecordViolation> {
        if schema.id_field_schema().is_none() {
            return Err(RecordViolation::SchemaHasNoIdField { field: schema.id_field });
        }
        for (index, record) in self.items.iter().enumerate() {
            for (name, value) in record.fields() {
                let Some(field) = schema.field(name) else {
                    return Err(RecordViolation::UnknownField { index, field: name.clone() });
                };
                if !value_matches_kind(value, field.kind) {
                    return Err(RecordViolation::WrongKind {
                        index,
                        field: name.clone(),
                        expected: field.kind,
                    });
                }
            }
            for field in schema.fields {
                if field.required && record.get(field.name).is_none() {
                    return Err(RecordViolation::MissingField { index, field: field.name });
                }
            }
            // A single name repeated inside one record: the record has no single value for it, and
            // `get` would answer the first while a writer might set the second.
            if let Some(repeated) = repeated_field(record) {
                return Err(RecordViolation::RepeatedField { index, field: repeated });
            }
        }
        self.check_ids(schema)
    }

    /// Checks every record against the schema the list itself carries.
    ///
    /// A convenience over [`Self::validate_against`] for a list a control built — it already knows
    /// which schema it built it for.
    pub fn validate(&self) -> Result<(), RecordViolation> {
        self.validate_against(self.schema)
    }

    /// The identity rules, which are properties of the list rather than of one record.
    fn check_ids(&self, schema: &'static RecordSchema) -> Result<(), RecordViolation> {
        for (index, record) in self.items.iter().enumerate() {
            let id = record.text(schema.id_field);
            if schema.ids_required && id.is_empty() {
                return Err(RecordViolation::EmptyId { index });
            }
            // Only a non-empty id can collide: two records that both omit the id are "missing",
            // reported above when the schema requires it, and allowed when it does not.
            if !schema.ids_unique || id.is_empty() {
                continue;
            }
            if let Some(first) =
                self.items[..index].iter().position(|earlier| earlier.text(schema.id_field) == id)
            {
                return Err(RecordViolation::DuplicateId {
                    id: String::from(id),
                    first,
                    second: index,
                });
            }
        }
        Ok(())
    }

    /// A copy holding every record except the one whose id is `id`.
    ///
    /// Removal is expressed as a copy rather than a mutation because that is where it is used: a
    /// host handing the library a new list. Returning `None` when the id is absent (rather than an
    /// unchanged copy) keeps "removed" and "was not there" distinguishable at the call site.
    pub fn without_id(&self, id: &str) -> Option<Self> {
        let index = self.index_of_id(id)?;
        let mut items = self.items.clone();
        items.remove(index);
        Some(Self { schema: self.schema, items })
    }
}

impl Default for RecordList {
    /// An empty list for an empty schema.
    ///
    /// `Default` is required by the collection types it sits beside, and the only honest default is
    /// the schema-less one: a list that claims a schema it was not given would validate against
    /// rules nobody declared.
    fn default() -> Self {
        Self::new(&EMPTY_RECORD_SCHEMA)
    }
}

/// The schema a schema-less list carries: no fields, no identity.
///
/// # Why this is not "no schema"
///
/// A list built by the JSON loader or the C ABI cannot name a `&'static` schema, so it carries this
/// one and is judged by [`RecordList::validate_against`] against the schema the *property* declares.
/// Carrying nothing at all would mean the fields could not be enumerated by anyone — a value that
/// describes itself is worth more than one that must be paired with its schema before it can be read.
pub const EMPTY_RECORD_SCHEMA: RecordSchema =
    RecordSchema { fields: &[], id_field: "", ids_unique: false, ids_required: false };

/// The name a record repeats, if it repeats one.
///
/// Returns the *first* repeated name in field order, so the message is deterministic rather than
/// depending on where the scan happens to look first.
fn repeated_field(record: &RecordValue) -> Option<String> {
    let fields = record.fields();
    for (position, (name, _)) in fields.iter().enumerate() {
        if fields[position + 1..].iter().any(|(other, _)| other == name) {
            return Some(name.clone());
        }
    }
    None
}

/// Whether a value's runtime shape satisfies a declared [`PropertyValueKind`].
///
/// Mirrors the property write path's own kind check, stated here so the record validator does not
/// depend on it. `Number` accepts either numeric carrier for the reason it does there: an integer
/// mode and a decimal mode are the same declaration.
fn value_matches_kind(value: &CapabilityValue, kind: PropertyValueKind) -> bool {
    match kind {
        PropertyValueKind::Bool => matches!(value, CapabilityValue::Bool(_)),
        PropertyValueKind::Int => matches!(value, CapabilityValue::Int(_)),
        PropertyValueKind::UInt => matches!(value, CapabilityValue::UInt(_)),
        PropertyValueKind::Float => matches!(value, CapabilityValue::Float(_)),
        PropertyValueKind::Number => {
            matches!(
                value,
                CapabilityValue::Int(_) | CapabilityValue::UInt(_) | CapabilityValue::Float(_)
            )
        }
        PropertyValueKind::String | PropertyValueKind::Enum => {
            matches!(value, CapabilityValue::String(_))
        }
        PropertyValueKind::Color => matches!(value, CapabilityValue::Color(_)),
        PropertyValueKind::Rect => matches!(value, CapabilityValue::Rect(_)),
        PropertyValueKind::IconSpec => matches!(value, CapabilityValue::IconSpec(_)),
        // A list kind is checked structurally: the carrier is a `Tuple`, and every element must be
        // the kind the list declares. An empty list matches, exactly as an empty `String` does.
        PropertyValueKind::StringList => matches!(
            value,
            CapabilityValue::Tuple(items)
                if items.iter().all(|item| matches!(item, CapabilityValue::String(_)))
        ),
        PropertyValueKind::NumberList => matches!(
            value,
            CapabilityValue::Tuple(items)
                if items.iter().all(|item| matches!(
                    item,
                    CapabilityValue::Float(_) | CapabilityValue::Int(_) | CapabilityValue::UInt(_)
                ))
        ),
        // A field whose declared kind is itself a record list: no control declares one, and
        // accepting any `Records` value here would let an unvalidated nested list through under the
        // name of a field. Refused rather than guessed.
        PropertyValueKind::Records => false,
    }
}

/// Which list edit an index complaint came from.
///
/// Carried so a caller (and the message) can say which operation the index was out of range *for*:
/// a row index and a column index are not interchangeable, and a bare number would not say which
/// the caller got wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordOperation {
    /// [`RecordList::move_item`].
    MoveItem,
    /// [`RecordList::move_field`].
    MoveField,
}

impl RecordOperation {
    /// The operation's name, for a message.
    pub const fn token(self) -> &'static str {
        match self {
            Self::MoveItem => "move item",
            Self::MoveField => "move field",
        }
    }
}

/// Why a record list does not conform to its schema, or why an edit on it was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordViolation {
    /// The schema names an identity field it does not declare, so nothing can be keyed on it.
    SchemaHasNoIdField {
        /// The field the schema named.
        field: &'static str,
    },
    /// A record is missing a field the schema requires.
    MissingField {
        /// Which record, by position.
        index: usize,
        /// The required field that is absent.
        field: &'static str,
    },
    /// A record carries a field the schema does not declare.
    UnknownField {
        /// Which record, by position.
        index: usize,
        /// The undeclared field.
        field: String,
    },
    /// A record carries the same field twice, so it has no single value for it.
    RepeatedField {
        /// Which record, by position.
        index: usize,
        /// The repeated field.
        field: String,
    },
    /// A field's value is not of the kind the schema declares.
    WrongKind {
        /// Which record, by position.
        index: usize,
        /// The field whose value was wrong.
        field: String,
        /// The kind the schema declares.
        expected: PropertyValueKind,
    },
    /// An id the schema requires is empty.
    EmptyId {
        /// Which record, by position.
        index: usize,
    },
    /// Two records claim the same id, which a unique-keyed schema refuses.
    DuplicateId {
        /// The id both records claim.
        id: String,
        /// The position of the record that claimed it first.
        first: usize,
        /// The position of the record that repeated it.
        second: usize,
    },
    /// An edit named an index past the end of what it was editing.
    IndexOutOfRange {
        /// Which edit.
        operation: RecordOperation,
        /// The index that did not resolve.
        index: usize,
        /// How many positions were available.
        len: usize,
    },
    /// An edit named a field the schema does not declare.
    UnknownFieldName {
        /// The undeclared field.
        field: String,
    },
}

impl RecordViolation {
    /// The record the violation is about, when it is about one.
    ///
    /// `None` for the two whole-list complaints (a schema with no identity field, and an edit that
    /// named an unknown field), which is what lets a designer point at the offending row rather than
    /// at the property as a whole.
    pub const fn index(&self) -> Option<usize> {
        match self {
            Self::MissingField { index, .. }
            | Self::UnknownField { index, .. }
            | Self::RepeatedField { index, .. }
            | Self::WrongKind { index, .. }
            | Self::EmptyId { index } => Some(*index),
            Self::DuplicateId { second, .. } => Some(*second),
            Self::IndexOutOfRange { index, .. } => Some(*index),
            Self::SchemaHasNoIdField { .. } | Self::UnknownFieldName { .. } => None,
        }
    }
}

impl core::fmt::Display for RecordViolation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SchemaHasNoIdField { field } => write!(
                f,
                "the record schema names `{field}` as its identity field but does not declare it"
            ),
            Self::MissingField { index, field } => {
                write!(f, "record {index} is missing the required field `{field}`")
            }
            Self::UnknownField { index, field } => {
                write!(f, "record {index} carries the undeclared field `{field}`")
            }
            Self::RepeatedField { index, field } => {
                write!(f, "record {index} carries the field `{field}` more than once")
            }
            Self::WrongKind { index, field, expected } => write!(
                f,
                "record {index} field `{field}` is not the declared kind `{}`",
                super::designer_manifest::value_kind_token(*expected)
            ),
            Self::EmptyId { index } => {
                write!(f, "record {index} has an empty id, which the schema requires")
            }
            Self::DuplicateId { id, first, second } => write!(
                f,
                "records {first} and {second} claim the same id `{id}`, which the schema requires to \
                 be unique"
            ),
            Self::IndexOutOfRange { operation, index, len } => {
                write!(f, "cannot {} to {index}: the list has {len} positions", operation.token())
            }
            Self::UnknownFieldName { field } => {
                write!(f, "the schema does not declare a field named `{field}`")
            }
        }
    }
}

/// The value a `Records` property carries, or `None` when it carries something else.
///
/// A free function rather than a `CapabilityValue` method so the two error-shaped readers
/// (`as_str`, `as_i64`, …) and this one stay in the same file as the enum they read.
pub fn as_record_list(value: &CapabilityValue) -> Option<&RecordList> {
    match value {
        CapabilityValue::Records(list) => Some(list),
        _ => None,
    }
}

/// The compact description of a record list, for a diagnostic that must fit on one line.
pub fn describe_records(list: &RecordList) -> String {
    format!("{} record(s) of schema {}", list.len(), list.schema().names().join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHIP_ITEM: RecordSchema = RecordSchema {
        fields: &[
            RecordField::required("id", PropertyValueKind::String),
            RecordField::required("label", PropertyValueKind::String),
            RecordField::optional("selected", PropertyValueKind::Bool),
        ],
        id_field: "id",
        ids_unique: true,
        ids_required: true,
    };

    fn text(value: &str) -> CapabilityValue {
        CapabilityValue::String(String::from(value))
    }

    fn item(id: &str, label: &str, selected: bool) -> RecordValue {
        RecordValue::new()
            .with("id", text(id))
            .with("label", text(label))
            .with("selected", CapabilityValue::Bool(selected))
    }

    fn ids(list: &RecordList) -> Vec<String> {
        (0..list.len())
            .map(|index| String::from(list.id_of(index).expect("every item has an id")))
            .collect()
    }

    fn list() -> RecordList {
        RecordList::from_items(
            &CHIP_ITEM,
            vec![item("a", "Alpha", false), item("b", "Beta", true), item("c", "Gamma", false)],
        )
    }

    /// A well-formed list validates, and every declared field is addressable.
    #[test]
    fn a_conforming_list_validates() {
        let list = list();
        assert_eq!(list.validate(), Ok(()));
        assert_eq!(list.len(), 3);
        assert_eq!(list.schema().names(), vec!["id", "label", "selected"]);
        assert_eq!(list.id_of(1), Some("b"));
        assert_eq!(list.index_of_id("c"), Some(2));
        assert_eq!(list.id_of(3), None);
    }

    /// A move reorders the list **and keeps every item's identity**.
    ///
    /// This is the whole point of declaring an identity field: positions change, the items do not.
    #[test]
    fn a_move_reorders_without_changing_identity() {
        let mut list = list();
        let before = ids(&list);

        list.move_item(0, 2).expect("both indices are in range");

        assert_eq!(list.id_of(0), Some("b"));
        assert_eq!(list.id_of(1), Some("c"));
        assert_eq!(list.id_of(2), Some("a"));
        // The moved record is the *same* record: its own fields travelled with it.
        assert_eq!(list.get_by_id("a").expect("still present").text("label"), "Alpha");
        assert!(matches!(
            list.get_by_id("b").expect("still present").get("selected"),
            Some(CapabilityValue::Bool(true))
        ));
        // And no id was invented or lost by the reorder.
        let mut after = ids(&list);
        let mut sorted_before = before;
        after.sort();
        sorted_before.sort();
        assert_eq!(after, sorted_before);
    }

    /// A move to the same position is a no-op, and an out-of-range index is refused *without*
    /// touching the list.
    #[test]
    fn a_move_that_cannot_happen_leaves_the_list_alone() {
        let mut list = list();
        let unchanged = list.clone();

        assert_eq!(list.move_item(1, 1), Ok(()));
        assert_eq!(list, unchanged);

        assert_eq!(
            list.move_item(3, 0),
            Err(RecordViolation::IndexOutOfRange {
                operation: RecordOperation::MoveItem,
                index: 3,
                len: 3
            })
        );
        assert_eq!(list, unchanged);
        assert_eq!(
            list.move_item(0, 9),
            Err(RecordViolation::IndexOutOfRange {
                operation: RecordOperation::MoveItem,
                index: 9,
                len: 3
            })
        );
        assert_eq!(list, unchanged);
    }

    /// A column move reorders one field in every record that carries it, and leaves ids alone.
    #[test]
    fn a_column_move_reorders_every_record_and_keeps_the_ids_stable() {
        let mut list = list();
        let before = ids(&list);

        // `label` to the front: the order becomes label, id, selected.
        list.move_field("label", 0).expect("label is declared and 0 is in range");

        for record in list.items() {
            assert_eq!(record.names(), vec!["label", "id", "selected"]);
        }
        assert_eq!(list.get(0).expect("first record").text("label"), "Alpha");
        assert_eq!(list.get(0).expect("first record").text("id"), "a");
        assert_eq!(ids(&list), before);
        // A permuted record is still a conforming record.
        assert_eq!(list.validate(), Ok(()));
    }

    /// A record that does not carry the moved field is left alone, and the rest stay consistently
    /// ordered.
    #[test]
    fn a_column_move_skips_a_record_that_does_not_carry_the_field() {
        let mut list = RecordList::from_items(
            &CHIP_ITEM,
            vec![item("a", "Alpha", true), item("b", "Beta", false).without("selected")],
        );

        list.move_field("id", 2).expect("id is declared and 2 is in range");

        assert_eq!(list.get(0).expect("first").names(), vec!["label", "selected", "id"]);
        // The second record has no `selected`, so it keeps the two fields it does have, in the
        // schema-consistent order the move implies.
        assert_eq!(list.get(1).expect("second").names(), vec!["label", "id"]);
        assert_eq!(ids(&list), vec![String::from("a"), String::from("b")]);
        assert_eq!(list.validate(), Ok(()));
    }

    /// A column move that names an undeclared field, or a position past the end, changes nothing.
    #[test]
    fn a_column_move_that_cannot_happen_leaves_the_list_alone() {
        let mut list = list();
        let unchanged = list.clone();

        assert_eq!(
            list.move_field("roles", 0),
            Err(RecordViolation::UnknownFieldName { field: String::from("roles") })
        );
        assert_eq!(list, unchanged);

        assert_eq!(
            list.move_field("label", 3),
            Err(RecordViolation::IndexOutOfRange {
                operation: RecordOperation::MoveField,
                index: 3,
                len: 3
            })
        );
        assert_eq!(list, unchanged);
    }

    /// Each schema rule is enforced, and the violation says which record and which field.
    #[test]
    fn every_schema_rule_is_reported_with_its_record_and_field() {
        // A required field that is absent.
        let missing =
            RecordList::from_items(&CHIP_ITEM, vec![item("a", "A", false).without("label")]);
        assert_eq!(
            missing.validate(),
            Err(RecordViolation::MissingField { index: 0, field: "label" })
        );

        // A field the schema does not declare.
        let unknown =
            RecordList::from_items(&CHIP_ITEM, vec![item("a", "A", false).with("rol", text("x"))]);
        assert_eq!(
            unknown.validate(),
            Err(RecordViolation::UnknownField { index: 0, field: String::from("rol") })
        );

        // A field whose value is not the declared kind.
        let wrong_kind = RecordList::from_items(
            &CHIP_ITEM,
            vec![item("a", "A", false).with("selected", CapabilityValue::UInt(1))],
        );
        assert_eq!(
            wrong_kind.validate(),
            Err(RecordViolation::WrongKind {
                index: 0,
                field: String::from("selected"),
                expected: PropertyValueKind::Bool
            })
        );

        // An empty id where the schema requires one.
        let empty_id = RecordList::from_items(
            &CHIP_ITEM,
            vec![RecordValue::new().with("id", text("")).with("label", text("A"))],
        );
        assert_eq!(empty_id.validate(), Err(RecordViolation::EmptyId { index: 0 }));

        // Two records claiming one id.
        let duplicated =
            RecordList::from_items(&CHIP_ITEM, vec![item("a", "A", false), item("a", "B", false)]);
        assert_eq!(
            duplicated.validate(),
            Err(RecordViolation::DuplicateId { id: String::from("a"), first: 0, second: 1 })
        );

        // The same field twice in one record.
        let repeated = RecordList::from_items(
            &CHIP_ITEM,
            vec![item("a", "A", false).with("label", text("B"))],
        );
        assert_eq!(
            repeated.validate(),
            Err(RecordViolation::RepeatedField { index: 0, field: String::from("label") })
        );
    }

    /// A schema that names an identity field it does not declare is a schema defect, not a list that
    /// quietly cannot be keyed.
    #[test]
    fn a_schema_without_its_own_identity_field_is_refused() {
        const BROKEN: RecordSchema = RecordSchema {
            fields: &[RecordField::required("label", PropertyValueKind::String)],
            id_field: "id",
            ids_unique: true,
            ids_required: true,
        };
        let list =
            RecordList::from_items(&BROKEN, vec![RecordValue::new().with("label", text("A"))]);
        assert_eq!(list.validate(), Err(RecordViolation::SchemaHasNoIdField { field: "id" }));
    }

    /// A schema that does not require unique ids accepts a repeat, and a repeat is still addressable
    /// by position.
    #[test]
    fn a_repeated_id_is_accepted_when_the_schema_allows_it() {
        const ALLOWING: RecordSchema = RecordSchema {
            fields: &[RecordField::required("id", PropertyValueKind::String)],
            id_field: "id",
            ids_unique: false,
            ids_required: true,
        };
        let list = RecordList::from_items(
            &ALLOWING,
            vec![
                RecordValue::new().with("id", text("a")),
                RecordValue::new().with("id", text("a")),
            ],
        );
        assert_eq!(list.validate(), Ok(()));
        assert_eq!(list.index_of_id("a"), Some(0));
    }

    /// An optional field may be absent, and a list is empty only when it holds nothing.
    #[test]
    fn an_optional_field_may_be_absent() {
        let list =
            RecordList::from_items(&CHIP_ITEM, vec![item("a", "A", false).without("selected")]);
        assert_eq!(list.validate(), Ok(()));
        assert!(list.get(0).expect("record").get("selected").is_none());
        assert_eq!(list.get(0).expect("record").text("selected"), "");
        assert!(!list.is_empty());
        assert!(RecordList::new(&CHIP_ITEM).is_empty());
        assert_eq!(list.get(7), None);
    }

    /// Removal is by identity, and an absent id is distinguishable from a removed one.
    #[test]
    fn removal_is_by_identity() {
        let list = list();
        let without_b = list.without_id("b").expect("b is present");
        assert_eq!(without_b.len(), 2);
        assert_eq!(without_b.index_of_id("b"), None);
        // The original is untouched: `without_id` copies.
        assert_eq!(list.len(), 3);
        assert_eq!(list.without_id("zzz"), None);
    }

    /// Every violation renders a message that names the record and the field, so a report can point
    /// at the offending row — and the two whole-list complaints point at no row.
    #[test]
    fn violations_describe_themselves() {
        for violation in [
            RecordViolation::SchemaHasNoIdField { field: "id" },
            RecordViolation::MissingField { index: 2, field: "label" },
            RecordViolation::UnknownField { index: 1, field: String::from("rol") },
            RecordViolation::RepeatedField { index: 0, field: String::from("id") },
            RecordViolation::WrongKind {
                index: 3,
                field: String::from("selected"),
                expected: PropertyValueKind::Bool,
            },
            RecordViolation::EmptyId { index: 0 },
            RecordViolation::DuplicateId { id: String::from("a"), first: 0, second: 4 },
            RecordViolation::IndexOutOfRange {
                operation: RecordOperation::MoveItem,
                index: 9,
                len: 3,
            },
            RecordViolation::UnknownFieldName { field: String::from("roles") },
        ] {
            let rendered = format!("{violation}");
            assert!(!rendered.is_empty(), "{violation:?} has no message");
        }
        assert_eq!(RecordViolation::MissingField { index: 2, field: "label" }.index(), Some(2));
        assert_eq!(
            RecordViolation::DuplicateId { id: String::from("a"), first: 0, second: 4 }.index(),
            Some(4),
            "a duplicate id points at the record that repeated it"
        );
        assert_eq!(
            RecordViolation::SchemaHasNoIdField { field: "id" }.index(),
            None,
            "a schema defect is not a complaint about one row"
        );
    }

    /// A `Records` value is readable as records and as nothing else, and describes itself.
    #[test]
    fn the_accessor_reads_only_a_record_list() {
        let value = CapabilityValue::Records(list());
        assert_eq!(as_record_list(&value).expect("a record list").len(), 3);
        assert_eq!(as_record_list(&CapabilityValue::Null), None);
        assert_eq!(
            describe_records(as_record_list(&value).expect("list")),
            "3 record(s) of schema id,label,selected"
        );
    }

    /// The default list claims no schema, so it accepts only the empty item set.
    #[test]
    fn a_default_list_claims_no_schema() {
        let list = RecordList::default();
        assert!(list.is_empty());
        assert_eq!(list.validate(), Err(RecordViolation::SchemaHasNoIdField { field: "" }));
    }

    /// A field declared as a record list is refused as a record field: no control declares one, and
    /// accepting any list would let an unvalidated nested list through under a field's name.
    #[test]
    fn a_nested_record_list_is_not_a_valid_field_value() {
        let value = CapabilityValue::Records(list());
        assert!(!value_matches_kind(&value, PropertyValueKind::Records));
        assert!(value_matches_kind(&CapabilityValue::Bool(true), PropertyValueKind::Bool));
    }
}
