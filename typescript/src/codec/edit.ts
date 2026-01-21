import { compareIds, type Id } from "../types/id.js";
import type { Context, ContextEdge, Edit, WireDictionaries } from "../types/edit.js";
import type { Op, UnsetLanguage } from "../types/op.js";
import { DataType, valueDataType, type PropertyValue } from "../types/value.js";
import { DecodeError, Reader, Writer } from "./primitives.js";
import { decodeOp, encodeOp, type OpDictionaryIndices, type OpDictionaryLookups } from "./op.js";

// Magic bytes
const MAGIC_UNCOMPRESSED = new TextEncoder().encode("GRC2");
const MAGIC_COMPRESSED = new TextEncoder().encode("GRC2Z");

// Current version
const VERSION = 0;

/**
 * Encoding options.
 */
export interface EncodeOptions {
  /** Use canonical encoding (deterministic, sorted dictionaries). */
  canonical?: boolean;
}

/**
 * Encodes an Edit to binary format.
 */
export function encodeEdit(edit: Edit, options?: EncodeOptions): Uint8Array {
  const canonical = options?.canonical ?? false;

  // Build dictionaries by scanning all ops (contexts are collected from ops)
  let dicts = buildDictionaries(edit.ops);

  // Sort dictionaries for canonical encoding
  if (canonical) {
    dicts = sortDictionaries(dicts);
  }

  // Create dictionary indices (with context collection support)
  const { indices, getContexts } = createDictionaryIndices(dicts);

  // First pass: encode ops to collect contexts
  const opsWriter = new Writer(edit.ops.length * 50);
  for (const op of edit.ops) {
    encodeOp(opsWriter, op, indices);
  }
  const opsBytes = opsWriter.finish();
  const contexts = getContexts();

  // Write to buffer
  const writer = new Writer(1024);

  // Magic + version
  writer.writeBytes(MAGIC_UNCOMPRESSED);
  writer.writeByte(VERSION);

  // Header
  writer.writeId(edit.id);
  writer.writeString(edit.name);

  // Authors (sorted for canonical)
  let authors = edit.authors;
  if (canonical) {
    authors = [...authors].sort(compareIds);
  }
  writer.writeIdVec(authors);
  writer.writeSignedVarint(edit.createdAt);

  // Dictionaries
  writeDictionaries(writer, dicts);

  // Contexts (collected from ops during encoding)
  writeContexts(writer, contexts, indices);

  // Operations (already encoded)
  writer.writeVarintNumber(edit.ops.length);
  writer.writeBytes(opsBytes);

  return writer.finish();
}

/**
 * Decodes binary data to an Edit.
 */
export function decodeEdit(data: Uint8Array): Edit {
  // Check for compression
  if (data.length >= 5 && matchesMagic(data, MAGIC_COMPRESSED)) {
    throw new DecodeError("E001", "compressed data detected - use decodeEditCompressed() or decompress first");
  }

  // Check magic
  if (data.length < 4 || !matchesMagic(data, MAGIC_UNCOMPRESSED)) {
    const found = data.length >= 4 ? data.subarray(0, 4) : data;
    throw new DecodeError("E001", `invalid magic bytes: expected GRC2, found ${Array.from(found)}`);
  }

  const reader = new Reader(data);

  // Skip magic
  reader.readBytes(4);

  // Version
  const version = reader.readByte();
  if (version !== VERSION) {
    throw new DecodeError("E001", `unsupported version: ${version}`);
  }

  // Header
  const id = reader.readId();
  const name = reader.readString();
  const authors = reader.readIdVec();
  const createdAt = reader.readSignedVarint();

  // Dictionaries
  const dicts = readDictionaries(reader);

  // Contexts - read and store in dicts for op decoding
  const contexts = readContexts(reader, dicts);
  dicts.contexts = contexts;
  const lookups = createDictionaryLookups(dicts);

  // Operations
  const opCount = reader.readVarintNumber();
  const ops: Op[] = [];
  for (let i = 0; i < opCount; i++) {
    ops.push(decodeOp(reader, lookups));
  }

  return { id, name, authors, createdAt, ops };
}

function matchesMagic(data: Uint8Array, magic: Uint8Array): boolean {
  for (let i = 0; i < magic.length; i++) {
    if (data[i] !== magic[i]) return false;
  }
  return true;
}

/**
 * Dictionary builder for encoding.
 */
interface DictionaryBuilder {
  properties: Map<string, { id: Id; dataType: DataType }>;
  relationTypes: Map<string, Id>;
  languages: Map<string, Id>;
  units: Map<string, Id>;
  objects: Map<string, Id>;
  contextIds: Map<string, Id>;
}

function idKey(id: Id): string {
  return Array.from(id).map(b => b.toString(16).padStart(2, '0')).join('');
}

function buildDictionaries(ops: Op[]): DictionaryBuilder {
  const dicts: DictionaryBuilder = {
    properties: new Map(),
    relationTypes: new Map(),
    languages: new Map(),
    units: new Map(),
    objects: new Map(),
    contextIds: new Map(),
  };

  // Note: contextIds are populated during encoding when addContext is called

  function addProperty(id: Id, dataType: DataType): void {
    const key = idKey(id);
    if (!dicts.properties.has(key)) {
      dicts.properties.set(key, { id, dataType });
    }
  }

  function addRelationType(id: Id): void {
    const key = idKey(id);
    if (!dicts.relationTypes.has(key)) {
      dicts.relationTypes.set(key, id);
    }
  }

  function addLanguage(id: Id): void {
    const key = idKey(id);
    if (!dicts.languages.has(key)) {
      dicts.languages.set(key, id);
    }
  }

  function addUnit(id: Id): void {
    const key = idKey(id);
    if (!dicts.units.has(key)) {
      dicts.units.set(key, id);
    }
  }

  function addObject(id: Id): void {
    const key = idKey(id);
    if (!dicts.objects.has(key)) {
      dicts.objects.set(key, id);
    }
  }

  function processPropertyValue(pv: PropertyValue): void {
    addProperty(pv.property, valueDataType(pv.value));
    if (pv.value.type === "text" && pv.value.language) {
      addLanguage(pv.value.language);
    }
    if (
      (pv.value.type === "int64" || pv.value.type === "float64" || pv.value.type === "decimal") &&
      pv.value.unit
    ) {
      addUnit(pv.value.unit);
    }
  }

  function processUnsetLanguage(lang: UnsetLanguage): void {
    if (lang.type === "specific") {
      addLanguage(lang.language);
    }
  }

  for (const op of ops) {
    switch (op.type) {
      case "createEntity":
        // ID is inline, not in object dict
        for (const pv of op.values) {
          processPropertyValue(pv);
        }
        break;

      case "updateEntity":
        addObject(op.id);
        for (const pv of op.set) {
          processPropertyValue(pv);
        }
        for (const u of op.unset) {
          addProperty(u.property, DataType.Text); // Assume TEXT for unset properties
          processUnsetLanguage(u.language);
        }
        break;

      case "deleteEntity":
      case "restoreEntity":
        addObject(op.id);
        break;

      case "createRelation":
        // For unique mode, compute the derived ID and add to objects if referenced later
        addRelationType(op.relationType);
        addObject(op.from);
        addObject(op.to);
        // Many mode ID is inline
        // Entity is inline if present
        break;

      case "updateRelation":
      case "deleteRelation":
      case "restoreRelation":
        addObject(op.id);
        break;
    }
  }

  return dicts;
}

function sortDictionaries(dicts: DictionaryBuilder): DictionaryBuilder {
  // Sort each dictionary by ID bytes
  const sortedProps = Array.from(dicts.properties.values()).sort((a, b) => compareIds(a.id, b.id));
  const sortedRelTypes = Array.from(dicts.relationTypes.values()).sort(compareIds);
  const sortedLangs = Array.from(dicts.languages.values()).sort(compareIds);
  const sortedUnits = Array.from(dicts.units.values()).sort(compareIds);
  const sortedObjects = Array.from(dicts.objects.values()).sort(compareIds);
  const sortedContextIds = Array.from(dicts.contextIds.values()).sort(compareIds);

  const sorted: DictionaryBuilder = {
    properties: new Map(),
    relationTypes: new Map(),
    languages: new Map(),
    units: new Map(),
    objects: new Map(),
    contextIds: new Map(),
  };

  for (const prop of sortedProps) {
    sorted.properties.set(idKey(prop.id), prop);
  }
  for (const id of sortedRelTypes) {
    sorted.relationTypes.set(idKey(id), id);
  }
  for (const id of sortedLangs) {
    sorted.languages.set(idKey(id), id);
  }
  for (const id of sortedUnits) {
    sorted.units.set(idKey(id), id);
  }
  for (const id of sortedObjects) {
    sorted.objects.set(idKey(id), id);
  }
  for (const id of sortedContextIds) {
    sorted.contextIds.set(idKey(id), id);
  }

  return sorted;
}

/**
 * Creates dictionary indices for encoding, with support for collecting contexts.
 */
function createDictionaryIndices(dicts: DictionaryBuilder): {
  indices: OpDictionaryIndices;
  getContexts: () => Context[];
} {
  const propToIndex = new Map<string, number>();
  const propToDataType = new Map<string, DataType>();
  const relTypeToIndex = new Map<string, number>();
  const langToIndex = new Map<string, number>();
  const unitToIndex = new Map<string, number>();
  const objToIndex = new Map<string, number>();
  const ctxIdToIndex = new Map<string, number>();

  // Context collection (built during encoding)
  const contexts: Context[] = [];
  const contextToIndex = new Map<string, number>();

  let i = 0;
  for (const [key, prop] of dicts.properties) {
    propToIndex.set(key, i++);
    propToDataType.set(key, prop.dataType);
  }

  i = 0;
  for (const key of dicts.relationTypes.keys()) {
    relTypeToIndex.set(key, i++);
  }

  i = 0;
  for (const key of dicts.languages.keys()) {
    langToIndex.set(key, i++);
  }

  i = 0;
  for (const key of dicts.units.keys()) {
    unitToIndex.set(key, i++);
  }

  i = 0;
  for (const key of dicts.objects.keys()) {
    objToIndex.set(key, i++);
  }

  i = 0;
  for (const key of dicts.contextIds.keys()) {
    ctxIdToIndex.set(key, i++);
  }

  // Helper to add a context ID (for contexts collected during encoding)
  function addContextId(id: Id): void {
    const key = idKey(id);
    if (!ctxIdToIndex.has(key)) {
      const idx = ctxIdToIndex.size;
      ctxIdToIndex.set(key, idx);
      dicts.contextIds.set(key, id);
    }
  }

  // Helper to add a relation type (for context edges)
  function addRelationType(id: Id): void {
    const key = idKey(id);
    if (!relTypeToIndex.has(key)) {
      const idx = relTypeToIndex.size;
      relTypeToIndex.set(key, idx);
      dicts.relationTypes.set(key, id);
    }
  }

  // Create a serializable key for context deduplication
  function contextKey(ctx: Context): string {
    const edgeKeys = ctx.edges.map(e => `${idKey(e.typeId)}:${idKey(e.toEntityId)}`).join(',');
    return `${idKey(ctx.rootId)}|${edgeKeys}`;
  }

  const indices: OpDictionaryIndices = {
    getPropertyIndex(id: Id): number {
      const key = idKey(id);
      const idx = propToIndex.get(key);
      if (idx === undefined) {
        throw new Error(`property not in dictionary: ${key}`);
      }
      return idx;
    },
    getLanguageIndex(id: Id | undefined): number {
      if (id === undefined) return 0;
      const key = idKey(id);
      const idx = langToIndex.get(key);
      if (idx === undefined) {
        throw new Error(`language not in dictionary: ${key}`);
      }
      return idx + 1;
    },
    getUnitIndex(id: Id | undefined): number {
      if (id === undefined) return 0;
      const key = idKey(id);
      const idx = unitToIndex.get(key);
      if (idx === undefined) {
        throw new Error(`unit not in dictionary: ${key}`);
      }
      return idx + 1;
    },
    getDataType(propertyId: Id): DataType {
      const key = idKey(propertyId);
      const dt = propToDataType.get(key);
      if (dt === undefined) {
        throw new Error(`property not in dictionary: ${key}`);
      }
      return dt;
    },
    getObjectIndex(id: Id): number {
      const key = idKey(id);
      const idx = objToIndex.get(key);
      if (idx === undefined) {
        throw new Error(`object not in dictionary: ${key}`);
      }
      return idx;
    },
    getRelationTypeIndex(id: Id): number {
      const key = idKey(id);
      const idx = relTypeToIndex.get(key);
      if (idx === undefined) {
        throw new Error(`relation type not in dictionary: ${key}`);
      }
      return idx;
    },
    getContextIdIndex(id: Id): number {
      const key = idKey(id);
      const idx = ctxIdToIndex.get(key);
      if (idx === undefined) {
        throw new Error(`context ID not in dictionary: ${key}`);
      }
      return idx;
    },
    addContext(ctx: Context): number {
      const key = contextKey(ctx);
      const existing = contextToIndex.get(key);
      if (existing !== undefined) {
        return existing;
      }
      // Register all IDs in the context
      addContextId(ctx.rootId);
      for (const edge of ctx.edges) {
        addRelationType(edge.typeId);
        addContextId(edge.toEntityId);
      }
      // Add to contexts array
      const idx = contexts.length;
      contexts.push(ctx);
      contextToIndex.set(key, idx);
      return idx;
    },
  };

  return { indices, getContexts: () => contexts };
}

function writeDictionaries(writer: Writer, dicts: DictionaryBuilder): void {
  // Properties: count + (id, data_type) pairs
  writer.writeVarintNumber(dicts.properties.size);
  for (const prop of dicts.properties.values()) {
    writer.writeId(prop.id);
    writer.writeByte(prop.dataType);
  }

  // Relation types
  writer.writeVarintNumber(dicts.relationTypes.size);
  for (const id of dicts.relationTypes.values()) {
    writer.writeId(id);
  }

  // Languages
  writer.writeVarintNumber(dicts.languages.size);
  for (const id of dicts.languages.values()) {
    writer.writeId(id);
  }

  // Units
  writer.writeVarintNumber(dicts.units.size);
  for (const id of dicts.units.values()) {
    writer.writeId(id);
  }

  // Objects
  writer.writeVarintNumber(dicts.objects.size);
  for (const id of dicts.objects.values()) {
    writer.writeId(id);
  }

  // Context IDs
  writer.writeVarintNumber(dicts.contextIds.size);
  for (const id of dicts.contextIds.values()) {
    writer.writeId(id);
  }
}

function writeContexts(writer: Writer, contexts: Context[], indices: OpDictionaryIndices): void {
  writer.writeVarintNumber(contexts.length);
  for (const ctx of contexts) {
    // root_id as ContextRef (index into contextIds)
    writer.writeVarintNumber(indices.getContextIdIndex(ctx.rootId));
    // edges
    writer.writeVarintNumber(ctx.edges.length);
    for (const edge of ctx.edges) {
      // type_id as RelationTypeRef
      writer.writeVarintNumber(indices.getRelationTypeIndex(edge.typeId));
      // to_entity_id as ContextRef
      writer.writeVarintNumber(indices.getContextIdIndex(edge.toEntityId));
    }
  }
}

function readDictionaries(reader: Reader): WireDictionaries {
  // Properties
  const propCount = reader.readVarintNumber();
  const properties: Array<{ id: Id; dataType: DataType }> = [];
  for (let i = 0; i < propCount; i++) {
    const id = reader.readId();
    const dataTypeByte = reader.readByte();
    if (dataTypeByte < 1 || dataTypeByte > 12) {
      throw new DecodeError("E005", `invalid data type: ${dataTypeByte}`);
    }
    properties.push({ id, dataType: dataTypeByte as DataType });
  }

  // Relation types
  const relationTypes = reader.readIdVec();

  // Languages
  const languages = reader.readIdVec();

  // Units
  const units = reader.readIdVec();

  // Objects
  const objects = reader.readIdVec();

  // Context IDs
  const contextIds = reader.readIdVec();

  return { properties, relationTypes, languages, units, objects, contextIds, contexts: [] };
}

function readContexts(reader: Reader, dicts: WireDictionaries): Context[] {
  const contextCount = reader.readVarintNumber();
  const contexts: Context[] = [];

  for (let i = 0; i < contextCount; i++) {
    // root_id as ContextRef
    const rootIdIndex = reader.readVarintNumber();
    if (rootIdIndex >= dicts.contextIds.length) {
      throw new DecodeError("E002", `context ID index ${rootIdIndex} out of bounds`);
    }
    const rootId = dicts.contextIds[rootIdIndex];

    // edges
    const edgeCount = reader.readVarintNumber();
    const edges: ContextEdge[] = [];
    for (let j = 0; j < edgeCount; j++) {
      // type_id as RelationTypeRef
      const typeIdIndex = reader.readVarintNumber();
      if (typeIdIndex >= dicts.relationTypes.length) {
        throw new DecodeError("E002", `relation type index ${typeIdIndex} out of bounds`);
      }
      const typeId = dicts.relationTypes[typeIdIndex];

      // to_entity_id as ContextRef
      const toEntityIdIndex = reader.readVarintNumber();
      if (toEntityIdIndex >= dicts.contextIds.length) {
        throw new DecodeError("E002", `context ID index ${toEntityIdIndex} out of bounds`);
      }
      const toEntityId = dicts.contextIds[toEntityIdIndex];

      edges.push({ typeId, toEntityId });
    }

    contexts.push({ rootId, edges });
  }

  return contexts;
}

function createDictionaryLookups(dicts: WireDictionaries): OpDictionaryLookups {
  return {
    getProperty(index: number) {
      if (index >= dicts.properties.length) {
        throw new DecodeError("E002", `property index ${index} out of bounds (size: ${dicts.properties.length})`);
      }
      return dicts.properties[index];
    },
    getLanguage(index: number): Id | undefined {
      if (index === 0) return undefined;
      const langIndex = index - 1;
      if (langIndex >= dicts.languages.length) {
        throw new DecodeError("E002", `language index ${index} out of bounds`);
      }
      return dicts.languages[langIndex];
    },
    getUnit(index: number): Id | undefined {
      if (index === 0) return undefined;
      const unitIndex = index - 1;
      if (unitIndex >= dicts.units.length) {
        throw new DecodeError("E002", `unit index ${index} out of bounds`);
      }
      return dicts.units[unitIndex];
    },
    getObject(index: number): Id {
      if (index >= dicts.objects.length) {
        throw new DecodeError("E002", `object index ${index} out of bounds (size: ${dicts.objects.length})`);
      }
      return dicts.objects[index];
    },
    getRelationType(index: number): Id {
      if (index >= dicts.relationTypes.length) {
        throw new DecodeError("E002", `relation type index ${index} out of bounds (size: ${dicts.relationTypes.length})`);
      }
      return dicts.relationTypes[index];
    },
    getContextId(index: number): Id {
      if (index >= dicts.contextIds.length) {
        throw new DecodeError("E002", `context ID index ${index} out of bounds (size: ${dicts.contextIds.length})`);
      }
      return dicts.contextIds[index];
    },
    getContext(index: number): Context | undefined {
      if (index >= dicts.contexts.length) {
        return undefined;
      }
      return dicts.contexts[index];
    },
  };
}
