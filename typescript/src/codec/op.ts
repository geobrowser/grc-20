import type { Id } from "../types/id.js";
import type {
  CreateEntity,
  CreateRelation,
  CreateValueRef,
  DeleteEntity,
  DeleteRelation,
  Op,
  RestoreEntity,
  RestoreRelation,
  UnsetLanguage,
  UnsetValue,
  UnsetRelationField,
  UpdateEntity,
  UpdateRelation,
} from "../types/op.js";
import {
  OP_TYPE_CREATE_ENTITY,
  OP_TYPE_CREATE_RELATION,
  OP_TYPE_CREATE_VALUE_REF,
  OP_TYPE_DELETE_ENTITY,
  OP_TYPE_DELETE_RELATION,
  OP_TYPE_RESTORE_ENTITY,
  OP_TYPE_RESTORE_RELATION,
  OP_TYPE_UPDATE_ENTITY,
  OP_TYPE_UPDATE_RELATION,
} from "../types/op.js";
import type { PropertyValue } from "../types/value.js";
import { DecodeError, Reader, Writer } from "./primitives.js";
import {
  decodePropertyValue,
  encodePropertyValue,
  type DictionaryIndices,
  type DictionaryLookups,
} from "./value.js";

/**
 * Extended dictionary indices for ops (includes objects).
 */
export interface OpDictionaryIndices extends DictionaryIndices {
  getObjectIndex(id: Id): number;
  getRelationTypeIndex(id: Id): number;
}

/**
 * Extended dictionary lookups for ops.
 */
export interface OpDictionaryLookups extends DictionaryLookups {
  getObject(index: number): Id;
  getRelationType(index: number): Id;
}

// UpdateEntity flags
const UPDATE_HAS_SET_PROPERTIES = 0x01;
const UPDATE_HAS_UNSET_VALUES = 0x02;

// CreateRelation flags
const RELATION_HAS_FROM_SPACE = 0x01;
const RELATION_HAS_FROM_VERSION = 0x02;
const RELATION_HAS_TO_SPACE = 0x04;
const RELATION_HAS_TO_VERSION = 0x08;
const RELATION_HAS_ENTITY = 0x10;
const RELATION_HAS_POSITION = 0x20;
const RELATION_FROM_IS_VALUE_REF = 0x40;
const RELATION_TO_IS_VALUE_REF = 0x80;

// CreateValueRef flags
const VALUE_REF_HAS_LANGUAGE = 0x01;
const VALUE_REF_HAS_SPACE = 0x02;

// UpdateRelation set flags
const UPDATE_REL_SET_FROM_SPACE = 0x01;
const UPDATE_REL_SET_FROM_VERSION = 0x02;
const UPDATE_REL_SET_TO_SPACE = 0x04;
const UPDATE_REL_SET_TO_VERSION = 0x08;
const UPDATE_REL_SET_POSITION = 0x10;

// UpdateRelation unset flags
const UPDATE_REL_UNSET_FROM_SPACE = 0x01;
const UPDATE_REL_UNSET_FROM_VERSION = 0x02;
const UPDATE_REL_UNSET_TO_SPACE = 0x04;
const UPDATE_REL_UNSET_TO_VERSION = 0x08;
const UPDATE_REL_UNSET_POSITION = 0x10;

/**
 * Encodes a single operation.
 */
export function encodeOp(writer: Writer, op: Op, dicts: OpDictionaryIndices): void {
  switch (op.type) {
    case "createEntity":
      encodeCreateEntity(writer, op, dicts);
      break;
    case "updateEntity":
      encodeUpdateEntity(writer, op, dicts);
      break;
    case "deleteEntity":
      encodeDeleteEntity(writer, op, dicts);
      break;
    case "restoreEntity":
      encodeRestoreEntity(writer, op, dicts);
      break;
    case "createRelation":
      encodeCreateRelation(writer, op, dicts);
      break;
    case "updateRelation":
      encodeUpdateRelation(writer, op, dicts);
      break;
    case "deleteRelation":
      encodeDeleteRelation(writer, op, dicts);
      break;
    case "restoreRelation":
      encodeRestoreRelation(writer, op, dicts);
      break;
    case "createValueRef":
      encodeCreateValueRef(writer, op, dicts);
      break;
  }
}

function encodeCreateEntity(writer: Writer, op: CreateEntity, dicts: OpDictionaryIndices): void {
  writer.writeByte(OP_TYPE_CREATE_ENTITY);
  writer.writeId(op.id);
  writer.writeVarintNumber(op.values.length);
  for (const value of op.values) {
    encodePropertyValue(writer, value, dicts);
  }
}

function encodeUpdateEntity(writer: Writer, op: UpdateEntity, dicts: OpDictionaryIndices): void {
  writer.writeByte(OP_TYPE_UPDATE_ENTITY);
  writer.writeVarintNumber(dicts.getObjectIndex(op.id));

  let flags = 0;
  if (op.set.length > 0) flags |= UPDATE_HAS_SET_PROPERTIES;
  if (op.unset.length > 0) flags |= UPDATE_HAS_UNSET_VALUES;
  writer.writeByte(flags);

  if (op.set.length > 0) {
    writer.writeVarintNumber(op.set.length);
    for (const value of op.set) {
      encodePropertyValue(writer, value, dicts);
    }
  }

  if (op.unset.length > 0) {
    writer.writeVarintNumber(op.unset.length);
    for (const u of op.unset) {
      encodeUnsetValue(writer, u, dicts);
    }
  }
}

function encodeUnsetValue(writer: Writer, unset: UnsetValue, dicts: DictionaryIndices): void {
  writer.writeVarintNumber(dicts.getPropertyIndex(unset.property));
  encodeUnsetLanguage(writer, unset.language, dicts);
}

function encodeUnsetLanguage(writer: Writer, lang: UnsetLanguage, dicts: DictionaryIndices): void {
  switch (lang.type) {
    case "all":
      writer.writeVarint(0xffffffffn); // 0xFFFFFFFF
      break;
    case "english":
      writer.writeVarintNumber(0);
      break;
    case "specific":
      writer.writeVarintNumber(dicts.getLanguageIndex(lang.language));
      break;
  }
}

function encodeDeleteEntity(writer: Writer, op: DeleteEntity, dicts: OpDictionaryIndices): void {
  writer.writeByte(OP_TYPE_DELETE_ENTITY);
  writer.writeVarintNumber(dicts.getObjectIndex(op.id));
}

function encodeRestoreEntity(writer: Writer, op: RestoreEntity, dicts: OpDictionaryIndices): void {
  writer.writeByte(OP_TYPE_RESTORE_ENTITY);
  writer.writeVarintNumber(dicts.getObjectIndex(op.id));
}

function encodeCreateRelation(writer: Writer, op: CreateRelation, dicts: OpDictionaryIndices): void {
  writer.writeByte(OP_TYPE_CREATE_RELATION);

  // Relation ID (always explicit)
  writer.writeId(op.id);

  // Type
  writer.writeVarintNumber(dicts.getRelationTypeIndex(op.relationType));

  // Flags
  let flags = 0;
  if (op.fromSpace) flags |= RELATION_HAS_FROM_SPACE;
  if (op.fromVersion) flags |= RELATION_HAS_FROM_VERSION;
  if (op.toSpace) flags |= RELATION_HAS_TO_SPACE;
  if (op.toVersion) flags |= RELATION_HAS_TO_VERSION;
  if (op.entity) flags |= RELATION_HAS_ENTITY;
  if (op.position) flags |= RELATION_HAS_POSITION;
  if (op.fromIsValueRef) flags |= RELATION_FROM_IS_VALUE_REF;
  if (op.toIsValueRef) flags |= RELATION_TO_IS_VALUE_REF;
  writer.writeByte(flags);

  // From endpoint: inline ID if value ref, otherwise ObjectRef
  if (op.fromIsValueRef) {
    writer.writeId(op.from);
  } else {
    writer.writeVarintNumber(dicts.getObjectIndex(op.from));
  }

  // To endpoint: inline ID if value ref, otherwise ObjectRef
  if (op.toIsValueRef) {
    writer.writeId(op.to);
  } else {
    writer.writeVarintNumber(dicts.getObjectIndex(op.to));
  }

  // Optional fields
  if (op.fromSpace) writer.writeId(op.fromSpace);
  if (op.fromVersion) writer.writeId(op.fromVersion);
  if (op.toSpace) writer.writeId(op.toSpace);
  if (op.toVersion) writer.writeId(op.toVersion);
  if (op.entity) writer.writeId(op.entity);
  if (op.position) writer.writeString(op.position);
}

function encodeUpdateRelation(writer: Writer, op: UpdateRelation, dicts: OpDictionaryIndices): void {
  writer.writeByte(OP_TYPE_UPDATE_RELATION);
  writer.writeVarintNumber(dicts.getObjectIndex(op.id));

  // Set flags
  let setFlags = 0;
  if (op.fromSpace !== undefined) setFlags |= UPDATE_REL_SET_FROM_SPACE;
  if (op.fromVersion !== undefined) setFlags |= UPDATE_REL_SET_FROM_VERSION;
  if (op.toSpace !== undefined) setFlags |= UPDATE_REL_SET_TO_SPACE;
  if (op.toVersion !== undefined) setFlags |= UPDATE_REL_SET_TO_VERSION;
  if (op.position !== undefined) setFlags |= UPDATE_REL_SET_POSITION;
  writer.writeByte(setFlags);

  // Unset flags
  let unsetFlags = 0;
  for (const field of op.unset) {
    switch (field) {
      case "fromSpace":
        unsetFlags |= UPDATE_REL_UNSET_FROM_SPACE;
        break;
      case "fromVersion":
        unsetFlags |= UPDATE_REL_UNSET_FROM_VERSION;
        break;
      case "toSpace":
        unsetFlags |= UPDATE_REL_UNSET_TO_SPACE;
        break;
      case "toVersion":
        unsetFlags |= UPDATE_REL_UNSET_TO_VERSION;
        break;
      case "position":
        unsetFlags |= UPDATE_REL_UNSET_POSITION;
        break;
    }
  }
  writer.writeByte(unsetFlags);

  // Write set field values
  if (op.fromSpace !== undefined) writer.writeId(op.fromSpace);
  if (op.fromVersion !== undefined) writer.writeId(op.fromVersion);
  if (op.toSpace !== undefined) writer.writeId(op.toSpace);
  if (op.toVersion !== undefined) writer.writeId(op.toVersion);
  if (op.position !== undefined) writer.writeString(op.position);
}

function encodeDeleteRelation(writer: Writer, op: DeleteRelation, dicts: OpDictionaryIndices): void {
  writer.writeByte(OP_TYPE_DELETE_RELATION);
  writer.writeVarintNumber(dicts.getObjectIndex(op.id));
}

function encodeRestoreRelation(writer: Writer, op: RestoreRelation, dicts: OpDictionaryIndices): void {
  writer.writeByte(OP_TYPE_RESTORE_RELATION);
  writer.writeVarintNumber(dicts.getObjectIndex(op.id));
}

function encodeCreateValueRef(writer: Writer, op: CreateValueRef, dicts: OpDictionaryIndices): void {
  writer.writeByte(OP_TYPE_CREATE_VALUE_REF);
  writer.writeId(op.id);
  writer.writeVarintNumber(dicts.getObjectIndex(op.entity));
  writer.writeVarintNumber(dicts.getPropertyIndex(op.property));

  let flags = 0;
  if (op.language !== undefined) flags |= VALUE_REF_HAS_LANGUAGE;
  if (op.space !== undefined) flags |= VALUE_REF_HAS_SPACE;
  writer.writeByte(flags);

  if (op.language !== undefined) {
    writer.writeVarintNumber(dicts.getLanguageIndex(op.language));
  }
  if (op.space !== undefined) {
    writer.writeId(op.space);
  }
}

/**
 * Decodes a single operation.
 */
export function decodeOp(reader: Reader, dicts: OpDictionaryLookups): Op {
  const opType = reader.readByte();

  switch (opType) {
    case OP_TYPE_CREATE_ENTITY:
      return decodeCreateEntity(reader, dicts);
    case OP_TYPE_UPDATE_ENTITY:
      return decodeUpdateEntity(reader, dicts);
    case OP_TYPE_DELETE_ENTITY:
      return decodeDeleteEntity(reader, dicts);
    case OP_TYPE_RESTORE_ENTITY:
      return decodeRestoreEntity(reader, dicts);
    case OP_TYPE_CREATE_RELATION:
      return decodeCreateRelation(reader, dicts);
    case OP_TYPE_UPDATE_RELATION:
      return decodeUpdateRelation(reader, dicts);
    case OP_TYPE_DELETE_RELATION:
      return decodeDeleteRelation(reader, dicts);
    case OP_TYPE_RESTORE_RELATION:
      return decodeRestoreRelation(reader, dicts);
    case OP_TYPE_CREATE_VALUE_REF:
      return decodeCreateValueRef(reader, dicts);
    default:
      throw new DecodeError("E005", `invalid op type: ${opType}`);
  }
}

function decodeCreateEntity(reader: Reader, dicts: OpDictionaryLookups): CreateEntity {
  const id = reader.readId();
  const valueCount = reader.readVarintNumber();
  const values = [];
  for (let i = 0; i < valueCount; i++) {
    values.push(decodePropertyValue(reader, dicts));
  }
  return { type: "createEntity", id, values };
}

function decodeUpdateEntity(reader: Reader, dicts: OpDictionaryLookups): UpdateEntity {
  const id = dicts.getObject(reader.readVarintNumber());
  const flags = reader.readByte();

  // Check reserved bits
  if ((flags & 0xfc) !== 0) {
    throw new DecodeError("E005", "reserved bits are non-zero in UpdateEntity flags");
  }

  const set: PropertyValue[] = [];
  if (flags & UPDATE_HAS_SET_PROPERTIES) {
    const count = reader.readVarintNumber();
    for (let i = 0; i < count; i++) {
      set.push(decodePropertyValue(reader, dicts));
    }
  }

  const unset: UnsetValue[] = [];
  if (flags & UPDATE_HAS_UNSET_VALUES) {
    const count = reader.readVarintNumber();
    for (let i = 0; i < count; i++) {
      unset.push(decodeUnsetValue(reader, dicts));
    }
  }

  return { type: "updateEntity", id, set, unset };
}

function decodeUnsetValue(reader: Reader, dicts: DictionaryLookups): UnsetValue {
  const propIndex = reader.readVarintNumber();
  const prop = dicts.getProperty(propIndex);
  const language = decodeUnsetLanguage(reader, dicts);
  return { property: prop.id, language };
}

function decodeUnsetLanguage(reader: Reader, dicts: DictionaryLookups): UnsetLanguage {
  const langValue = reader.readVarint();
  if (langValue === 0xffffffffn) {
    return { type: "all" };
  } else if (langValue === 0n) {
    return { type: "english" };
  } else {
    const language = dicts.getLanguage(Number(langValue));
    if (!language) {
      throw new DecodeError("E002", `language index ${langValue} out of bounds`);
    }
    return { type: "specific", language };
  }
}

function decodeDeleteEntity(reader: Reader, dicts: OpDictionaryLookups): DeleteEntity {
  const id = dicts.getObject(reader.readVarintNumber());
  return { type: "deleteEntity", id };
}

function decodeRestoreEntity(reader: Reader, dicts: OpDictionaryLookups): RestoreEntity {
  const id = dicts.getObject(reader.readVarintNumber());
  return { type: "restoreEntity", id };
}

function decodeCreateRelation(reader: Reader, dicts: OpDictionaryLookups): CreateRelation {
  // Relation ID (always explicit)
  const id = reader.readId();

  const relationType = dicts.getRelationType(reader.readVarintNumber());
  const flags = reader.readByte();

  const fromIsValueRef = (flags & RELATION_FROM_IS_VALUE_REF) !== 0;
  const toIsValueRef = (flags & RELATION_TO_IS_VALUE_REF) !== 0;

  // Read from endpoint: inline ID if value ref, otherwise ObjectRef
  const from = fromIsValueRef ? reader.readId() : dicts.getObject(reader.readVarintNumber());

  // Read to endpoint: inline ID if value ref, otherwise ObjectRef
  const to = toIsValueRef ? reader.readId() : dicts.getObject(reader.readVarintNumber());

  const fromSpace = flags & RELATION_HAS_FROM_SPACE ? reader.readId() : undefined;
  const fromVersion = flags & RELATION_HAS_FROM_VERSION ? reader.readId() : undefined;
  const toSpace = flags & RELATION_HAS_TO_SPACE ? reader.readId() : undefined;
  const toVersion = flags & RELATION_HAS_TO_VERSION ? reader.readId() : undefined;
  const entity = flags & RELATION_HAS_ENTITY ? reader.readId() : undefined;
  const position = flags & RELATION_HAS_POSITION ? reader.readString() : undefined;

  return {
    type: "createRelation",
    id,
    relationType,
    from,
    fromIsValueRef: fromIsValueRef || undefined,
    to,
    toIsValueRef: toIsValueRef || undefined,
    fromSpace,
    fromVersion,
    toSpace,
    toVersion,
    entity,
    position,
  };
}

function decodeUpdateRelation(reader: Reader, dicts: OpDictionaryLookups): UpdateRelation {
  const id = dicts.getObject(reader.readVarintNumber());

  // Read set flags
  const setFlags = reader.readByte();
  // Check reserved bits in set flags
  if ((setFlags & 0xe0) !== 0) {
    throw new DecodeError("E005", "reserved bits are non-zero in UpdateRelation set flags");
  }

  // Read unset flags
  const unsetFlags = reader.readByte();
  // Check reserved bits in unset flags
  if ((unsetFlags & 0xe0) !== 0) {
    throw new DecodeError("E005", "reserved bits are non-zero in UpdateRelation unset flags");
  }

  // Read set field values
  const fromSpace = setFlags & UPDATE_REL_SET_FROM_SPACE ? reader.readId() : undefined;
  const fromVersion = setFlags & UPDATE_REL_SET_FROM_VERSION ? reader.readId() : undefined;
  const toSpace = setFlags & UPDATE_REL_SET_TO_SPACE ? reader.readId() : undefined;
  const toVersion = setFlags & UPDATE_REL_SET_TO_VERSION ? reader.readId() : undefined;
  const position = setFlags & UPDATE_REL_SET_POSITION ? reader.readString() : undefined;

  // Decode unset fields
  const unset: UnsetRelationField[] = [];
  if (unsetFlags & UPDATE_REL_UNSET_FROM_SPACE) unset.push("fromSpace");
  if (unsetFlags & UPDATE_REL_UNSET_FROM_VERSION) unset.push("fromVersion");
  if (unsetFlags & UPDATE_REL_UNSET_TO_SPACE) unset.push("toSpace");
  if (unsetFlags & UPDATE_REL_UNSET_TO_VERSION) unset.push("toVersion");
  if (unsetFlags & UPDATE_REL_UNSET_POSITION) unset.push("position");

  return {
    type: "updateRelation",
    id,
    fromSpace,
    fromVersion,
    toSpace,
    toVersion,
    position,
    unset,
  };
}

function decodeDeleteRelation(reader: Reader, dicts: OpDictionaryLookups): DeleteRelation {
  const id = dicts.getObject(reader.readVarintNumber());
  return { type: "deleteRelation", id };
}

function decodeRestoreRelation(reader: Reader, dicts: OpDictionaryLookups): RestoreRelation {
  const id = dicts.getObject(reader.readVarintNumber());
  return { type: "restoreRelation", id };
}

function decodeCreateValueRef(reader: Reader, dicts: OpDictionaryLookups): CreateValueRef {
  const id = reader.readId();
  const entity = dicts.getObject(reader.readVarintNumber());
  const prop = dicts.getProperty(reader.readVarintNumber());
  const property = prop.id;

  const flags = reader.readByte();

  // Check reserved bits
  if ((flags & 0xfc) !== 0) {
    throw new DecodeError("E005", "reserved bits are non-zero in CreateValueRef flags");
  }

  const language = flags & VALUE_REF_HAS_LANGUAGE ? dicts.getLanguage(reader.readVarintNumber()) : undefined;
  const space = flags & VALUE_REF_HAS_SPACE ? reader.readId() : undefined;

  return {
    type: "createValueRef",
    id,
    entity,
    property,
    language,
    space,
  };
}
