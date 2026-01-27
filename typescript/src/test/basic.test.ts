import { describe, it, expect } from "vitest";
import type { Edit } from "../index.js";
import {
  EditBuilder,
  createEdit,
  createEntity,
  createRelation,
  updateEntity,
  deleteEntity,
  restoreEntity,
  updateRelation,
  deleteRelation,
  restoreRelation,
  createValueRef,
  encodeEdit,
  decodeEdit,
  encodeEditCompressed,
  decodeEditCompressed,
  encodeEditAuto,
  decodeEditAuto,
  isCompressed,
  preloadCompression,
  isCompressionReady,
  parseId,
  formatId,
  randomId,
  derivedUuid,
  derivedUuidFromString,
  uniqueRelationId,
  relationEntityId,
  properties,
  types,
  relationTypes,
  languages,
  idsEqual,
} from "../index.js";

describe("ID utilities", () => {
  it("formatId produces 32 hex chars", () => {
    const id = randomId();
    const hex = formatId(id);
    expect(hex.length).toBe(32);
    expect(/^[0-9a-f]{32}$/.test(hex)).toBe(true);
  });

  it("parseId roundtrips with formatId", () => {
    const id = randomId();
    const hex = formatId(id);
    const parsed = parseId(hex);
    expect(parsed).toBeDefined();
    expect(idsEqual(id, parsed!)).toBe(true);
  });

  it("parseId accepts hyphens", () => {
    const withHyphens = "550e8400-e29b-41d4-a716-446655440000";
    const withoutHyphens = "550e8400e29b41d4a716446655440000";

    const id1 = parseId(withHyphens);
    const id2 = parseId(withoutHyphens);

    expect(id1).toBeDefined();
    expect(id2).toBeDefined();
    expect(idsEqual(id1!, id2!)).toBe(true);
  });

  it("derivedUuid is deterministic", () => {
    const input = new TextEncoder().encode("test");
    const id1 = derivedUuid(input);
    const id2 = derivedUuid(input);
    expect(idsEqual(id1, id2)).toBe(true);
  });

  it("derivedUuid produces valid UUIDv8", () => {
    const id = derivedUuidFromString("test");
    // Version 8 in byte 6
    expect((id[6] & 0xf0)).toBe(0x80);
    // Variant in byte 8
    expect((id[8] & 0xc0)).toBe(0x80);
  });

  it("uniqueRelationId is deterministic", () => {
    const from = parseId("11111111111111111111111111111111")!;
    const to = parseId("22222222222222222222222222222222")!;
    const type = parseId("33333333333333333333333333333333")!;

    const id1 = uniqueRelationId(from, to, type);
    const id2 = uniqueRelationId(from, to, type);
    expect(idsEqual(id1, id2)).toBe(true);

    // Different from -> different id
    const id3 = uniqueRelationId(to, from, type);
    expect(idsEqual(id1, id3)).toBe(false);
  });

  it("relationEntityId differs from relation id", () => {
    const relationId = randomId();
    const entityId = relationEntityId(relationId);
    expect(idsEqual(relationId, entityId)).toBe(false);
  });
});

describe("Genesis IDs", () => {
  it("properties.NAME matches spec", () => {
    const id = properties.NAME;
    expect(formatId(id)).toBe("a126ca530c8e48d5b88882c734c38935");
    expect(idsEqual(id, properties.name())).toBe(true);
  });

  it("properties.DESCRIPTION matches spec", () => {
    const id = properties.DESCRIPTION;
    expect(formatId(id)).toBe("9b1f76ff9711404c861e59dc3fa7d037");
  });

  it("properties.COVER matches spec", () => {
    const id = properties.COVER;
    expect(formatId(id)).toBe("34f535072e6b42c5a84443981a77cfa2");
  });

  it("types.IMAGE matches spec", () => {
    const id = types.IMAGE;
    expect(formatId(id)).toBe("f3f790c4c74e4d23a0a91e8ef84e30d9");
  });

  it("relationTypes.TYPES matches spec", () => {
    const id = relationTypes.TYPES;
    expect(formatId(id)).toBe("8f151ba4de204e3c9cb499ddf96f48f1");
    expect(idsEqual(id, relationTypes.types())).toBe(true);
  });

  it("languages.english is deterministic", () => {
    const id1 = languages.english();
    const id2 = languages.fromCode("en");
    expect(idsEqual(id1, id2)).toBe(true);
  });
});

describe("EditBuilder", () => {
  it("creates a simple edit with entity", () => {
    const editId = randomId();
    const entityId = randomId();
    const authorId = randomId();

    const edit = new EditBuilder(editId)
      .setName("Test Edit")
      .addAuthor(authorId)
      .setCreatedAt(1234567890n)
      .createEntity(entityId, (e) =>
        e.text(properties.name(), "Alice", undefined)
         .int64(parseId("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")!, 42n, undefined)
      )
      .build();

    expect(idsEqual(edit.id, editId)).toBe(true);
    expect(edit.name).toBe("Test Edit");
    expect(edit.authors.length).toBe(1);
    expect(idsEqual(edit.authors[0], authorId)).toBe(true);
    expect(edit.createdAt).toBe(1234567890n);
    expect(edit.ops.length).toBe(1);
    expect(edit.ops[0].type).toBe("createEntity");
  });

  it("creates relations with explicit IDs", () => {
    const editId = randomId();
    const from = randomId();
    const to = randomId();
    const relId1 = randomId();
    const relId2 = randomId();

    const edit = new EditBuilder(editId)
      .createEmptyEntity(from)
      .createEmptyEntity(to)
      .createRelationSimple(relId1, from, to, relationTypes.types())
      .createRelationSimple(relId2, from, to, relationTypes.types())
      .build();

    expect(edit.ops.length).toBe(4);
    expect(edit.ops[2].type).toBe("createRelation");
    expect(edit.ops[3].type).toBe("createRelation");

    const rel1 = edit.ops[2];
    const rel2 = edit.ops[3];
    if (rel1.type === "createRelation" && rel2.type === "createRelation") {
      expect(idsEqual(rel1.id, relId1)).toBe(true);
      expect(idsEqual(rel2.id, relId2)).toBe(true);
    }
  });

  it("creates update entity operations", () => {
    const editId = randomId();
    const entityId = randomId();
    const propId = randomId();

    const edit = new EditBuilder(editId)
      .updateEntity(entityId, (u) =>
        u.setText(propId, "New value", undefined)
         .unsetAll(properties.description())
      )
      .build();

    expect(edit.ops.length).toBe(1);
    expect(edit.ops[0].type).toBe("updateEntity");

    const op = edit.ops[0];
    if (op.type === "updateEntity") {
      expect(op.set.length).toBe(1);
      expect(op.unset.length).toBe(1);
    }
  });
});

describe("Ops helpers", () => {
  it("creates edits from op factories", () => {
    const editId = randomId();
    const entityId = randomId();
    const authorId = randomId();
    const propId = randomId();
    const relationId = randomId();
    const from = randomId();
    const to = randomId();

    const ops = [
      createEntity({
        id: entityId,
        values: [{ property: propId, value: { type: "bool", value: true } }],
      }),
      createRelation({
        id: relationId,
        relationType: relationTypes.types(),
        from,
        to,
      }),
      updateEntity({
        id: entityId,
        set: [
          {
            property: properties.description(),
            value: { type: "text", value: "Updated" },
          },
        ],
      }),
    ];

    const edit = createEdit({
      id: editId,
      name: "Ops Edit",
      author: authorId,
      createdAt: 10n,
      ops,
    });

    expect(idsEqual(edit.id, editId)).toBe(true);
    expect(edit.name).toBe("Ops Edit");
    expect(edit.authors.length).toBe(1);
    expect(idsEqual(edit.authors[0], authorId)).toBe(true);
    expect(edit.createdAt).toBe(10n);
    expect(edit.ops.length).toBe(3);
    expect(edit.ops[0].type).toBe("createEntity");
  });

  it("creates deleteEntity operations", () => {
    const entityId = randomId();

    const op = deleteEntity(entityId);

    expect(op.type).toBe("deleteEntity");
    expect(idsEqual(op.id, entityId)).toBe(true);
  });

  it("creates restoreEntity operations", () => {
    const entityId = randomId();

    const op = restoreEntity(entityId);

    expect(op.type).toBe("restoreEntity");
    expect(idsEqual(op.id, entityId)).toBe(true);
  });

  it("creates updateRelation operations", () => {
    const relationId = randomId();

    const op = updateRelation({
      id: relationId,
      position: "newpos",
    });

    expect(op.type).toBe("updateRelation");
    expect(idsEqual(op.id, relationId)).toBe(true);
    expect(op.position).toBe("newpos");
    expect(op.unset).toEqual([]);
  });

  it("creates deleteRelation operations", () => {
    const relationId = randomId();

    const op = deleteRelation(relationId);

    expect(op.type).toBe("deleteRelation");
    expect(idsEqual(op.id, relationId)).toBe(true);
  });

  it("creates restoreRelation operations", () => {
    const relationId = randomId();

    const op = restoreRelation(relationId);

    expect(op.type).toBe("restoreRelation");
    expect(idsEqual(op.id, relationId)).toBe(true);
  });

  it("creates createValueRef operations", () => {
    const refId = randomId();
    const entityId = randomId();
    const propId = randomId();
    const langId = randomId();
    const spaceId = randomId();

    const op = createValueRef({
      id: refId,
      entity: entityId,
      property: propId,
      language: langId,
      space: spaceId,
    });

    expect(op.type).toBe("createValueRef");
    expect(idsEqual(op.id, refId)).toBe(true);
    expect(idsEqual(op.entity, entityId)).toBe(true);
    expect(idsEqual(op.property, propId)).toBe(true);
    expect(idsEqual(op.language!, langId)).toBe(true);
    expect(idsEqual(op.space!, spaceId)).toBe(true);
  });

  it("creates createValueRef without optional fields", () => {
    const refId = randomId();
    const entityId = randomId();
    const propId = randomId();

    const op = createValueRef({
      id: refId,
      entity: entityId,
      property: propId,
    });

    expect(op.type).toBe("createValueRef");
    expect(idsEqual(op.id, refId)).toBe(true);
    expect(op.language).toBeUndefined();
    expect(op.space).toBeUndefined();
  });
});

describe("Builder vs Ops API Equivalence", () => {
  it("produces identical encoding for createEntity", () => {
    const editId = parseId("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")!;
    const entityId = parseId("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")!;

    // Using EditBuilder
    const builderEdit = new EditBuilder(editId)
      .setCreatedAt(1000n)
      .createEntity(entityId, (e) =>
        e.text(properties.name(), "Test", undefined)
      )
      .build();

    // Using Ops API
    const opsEdit = createEdit({
      id: editId,
      createdAt: 1000n,
      ops: [
        createEntity({
          id: entityId,
          values: [
            { property: properties.name(), value: { type: "text", value: "Test" } },
          ],
        }),
      ],
    });

    const builderEncoded = encodeEdit(builderEdit, { canonical: true });
    const opsEncoded = encodeEdit(opsEdit, { canonical: true });

    expect(builderEncoded.length).toBe(opsEncoded.length);
    for (let i = 0; i < builderEncoded.length; i++) {
      expect(builderEncoded[i]).toBe(opsEncoded[i]);
    }
  });

  it("produces identical encoding for updateEntity", () => {
    const editId = parseId("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")!;
    const entityId = parseId("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")!;
    const propId = parseId("cccccccccccccccccccccccccccccccc")!;

    // Using EditBuilder
    const builderEdit = new EditBuilder(editId)
      .setCreatedAt(1000n)
      .updateEntity(entityId, (u) =>
        u.setText(propId, "Updated", undefined)
         .unsetAll(properties.description())
      )
      .build();

    // Using Ops API
    const opsEdit = createEdit({
      id: editId,
      createdAt: 1000n,
      ops: [
        updateEntity({
          id: entityId,
          set: [{ property: propId, value: { type: "text", value: "Updated" } }],
          unset: [{ property: properties.description(), language: { type: "all" } }],
        }),
      ],
    });

    const builderEncoded = encodeEdit(builderEdit, { canonical: true });
    const opsEncoded = encodeEdit(opsEdit, { canonical: true });

    expect(builderEncoded.length).toBe(opsEncoded.length);
    for (let i = 0; i < builderEncoded.length; i++) {
      expect(builderEncoded[i]).toBe(opsEncoded[i]);
    }
  });

  it("produces identical encoding for createRelation", () => {
    const editId = parseId("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")!;
    const relationId = parseId("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")!;
    const from = parseId("cccccccccccccccccccccccccccccccc")!;
    const to = parseId("dddddddddddddddddddddddddddddddd")!;

    // Using EditBuilder
    const builderEdit = new EditBuilder(editId)
      .setCreatedAt(1000n)
      .createRelationSimple(relationId, from, to, relationTypes.types())
      .build();

    // Using Ops API
    const opsEdit = createEdit({
      id: editId,
      createdAt: 1000n,
      ops: [
        createRelation({
          id: relationId,
          relationType: relationTypes.types(),
          from,
          to,
        }),
      ],
    });

    const builderEncoded = encodeEdit(builderEdit, { canonical: true });
    const opsEncoded = encodeEdit(opsEdit, { canonical: true });

    expect(builderEncoded.length).toBe(opsEncoded.length);
    for (let i = 0; i < builderEncoded.length; i++) {
      expect(builderEncoded[i]).toBe(opsEncoded[i]);
    }
  });

  it("produces identical encoding for delete/restore operations", () => {
    const editId = parseId("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")!;
    const entityId = parseId("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")!;
    const relationId = parseId("cccccccccccccccccccccccccccccccc")!;

    // Using EditBuilder
    const builderEdit = new EditBuilder(editId)
      .setCreatedAt(1000n)
      .deleteEntity(entityId)
      .restoreEntity(entityId)
      .deleteRelation(relationId)
      .restoreRelation(relationId)
      .build();

    // Using Ops API
    const opsEdit = createEdit({
      id: editId,
      createdAt: 1000n,
      ops: [
        deleteEntity(entityId),
        restoreEntity(entityId),
        deleteRelation(relationId),
        restoreRelation(relationId),
      ],
    });

    const builderEncoded = encodeEdit(builderEdit, { canonical: true });
    const opsEncoded = encodeEdit(opsEdit, { canonical: true });

    expect(builderEncoded.length).toBe(opsEncoded.length);
    for (let i = 0; i < builderEncoded.length; i++) {
      expect(builderEncoded[i]).toBe(opsEncoded[i]);
    }
  });

  it("produces identical encoding for complex multi-op edits", () => {
    const editId = parseId("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")!;
    const authorId = parseId("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")!;
    const entity1 = parseId("11111111111111111111111111111111")!;
    const entity2 = parseId("22222222222222222222222222222222")!;
    const relationId = parseId("33333333333333333333333333333333")!;
    const propId = parseId("44444444444444444444444444444444")!;

    // Using EditBuilder
    const builderEdit = new EditBuilder(editId)
      .setName("Complex Edit")
      .addAuthor(authorId)
      .setCreatedAt(1000n)
      .createEntity(entity1, (e) =>
        e.text(properties.name(), "Entity One", undefined)
         .bool(propId, true)
      )
      .createEntity(entity2, (e) =>
        e.text(properties.name(), "Entity Two", undefined)
      )
      .createRelationSimple(relationId, entity1, entity2, relationTypes.types())
      .updateEntity(entity1, (u) =>
        u.setText(properties.description(), "Updated description", undefined)
      )
      .build();

    // Using Ops API
    const opsEdit = createEdit({
      id: editId,
      name: "Complex Edit",
      author: authorId,
      createdAt: 1000n,
      ops: [
        createEntity({
          id: entity1,
          values: [
            { property: properties.name(), value: { type: "text", value: "Entity One" } },
            { property: propId, value: { type: "bool", value: true } },
          ],
        }),
        createEntity({
          id: entity2,
          values: [
            { property: properties.name(), value: { type: "text", value: "Entity Two" } },
          ],
        }),
        createRelation({
          id: relationId,
          relationType: relationTypes.types(),
          from: entity1,
          to: entity2,
        }),
        updateEntity({
          id: entity1,
          set: [
            { property: properties.description(), value: { type: "text", value: "Updated description" } },
          ],
        }),
      ],
    });

    const builderEncoded = encodeEdit(builderEdit, { canonical: true });
    const opsEncoded = encodeEdit(opsEdit, { canonical: true });

    expect(builderEncoded.length).toBe(opsEncoded.length);
    for (let i = 0; i < builderEncoded.length; i++) {
      expect(builderEncoded[i]).toBe(opsEncoded[i]);
    }
  });

  it("produces identical encoding for updateRelation", () => {
    const editId = parseId("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")!;
    const relationId = parseId("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")!;

    // Using EditBuilder
    const builderEdit = new EditBuilder(editId)
      .setCreatedAt(1000n)
      .updateRelation(relationId, (u) => u.setPosition("abc"))
      .build();

    // Using Ops API
    const opsEdit = createEdit({
      id: editId,
      createdAt: 1000n,
      ops: [
        updateRelation({
          id: relationId,
          position: "abc",
        }),
      ],
    });

    const builderEncoded = encodeEdit(builderEdit, { canonical: true });
    const opsEncoded = encodeEdit(opsEdit, { canonical: true });

    expect(builderEncoded.length).toBe(opsEncoded.length);
    for (let i = 0; i < builderEncoded.length; i++) {
      expect(builderEncoded[i]).toBe(opsEncoded[i]);
    }
  });
});

describe("Codec", () => {
  it("throws when encoding an ID with wrong length", () => {
    const editId = randomId();
    const entityId = randomId();

    // Create an edit with a malformed author ID (43 bytes instead of 16)
    const malformedAuthorId = new TextEncoder().encode(
      "*0x635bd835b6f6f9f1E9fB118C60811D9DBc704635"
    ) as unknown as ReturnType<typeof randomId>;

    const edit = new EditBuilder(editId)
      .setName("Test Edit")
      .addAuthor(malformedAuthorId)
      .createEntity(entityId, (e) =>
        e.text(properties.name(), "Test", undefined)
      )
      .build();

    expect(() => encodeEdit(edit)).toThrow("[E005] invalid id for edit.authors[0]");
  });

  it("throws when encoding a context with wrong length rootId", () => {
    const editId = randomId();
    const entityId = randomId();

    // Create a malformed context rootId (43 bytes instead of 16)
    const malformedRootId = new TextEncoder().encode(
      "*0x635bd835b6f6f9f1E9fB118C60811D9DBc704635"
    ) as unknown as ReturnType<typeof randomId>;

    const edit: Edit = {
      id: editId,
      name: "Test Edit",
      authors: [],
      createdAt: 0n,
      ops: [
        {
          type: "createEntity",
          id: entityId,
          values: [],
          context: {
            rootId: malformedRootId,
            edges: [],
          },
        },
      ],
    };

    expect(() => encodeEdit(edit)).toThrow("[E005] invalid id for op[0].context.rootId");
  });

  it("encodes and decodes a simple edit", () => {
    const editId = randomId();
    const entityId = randomId();

    const edit = new EditBuilder(editId)
      .setName("Test Edit")
      .setCreatedAt(1234567890000000n)
      .createEntity(entityId, (e) =>
        e.text(properties.name(), "Alice", undefined)
         .bool(parseId("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")!, true)
      )
      .build();

    const encoded = encodeEdit(edit);
    expect(encoded.length).toBeGreaterThan(0);

    // Check magic bytes
    expect(String.fromCharCode(...encoded.slice(0, 4))).toBe("GRC2");

    const decoded = decodeEdit(encoded);

    expect(idsEqual(decoded.id, edit.id)).toBe(true);
    expect(decoded.name).toBe(edit.name);
    expect(decoded.createdAt).toBe(edit.createdAt);
    expect(decoded.ops.length).toBe(edit.ops.length);
  });

  it("throws when updateEntity set and unset overlap", () => {
    const editId = randomId();
    const entityId = randomId();
    const propId = properties.name();

    const edit: Edit = {
      id: editId,
      name: "Test Edit",
      authors: [],
      createdAt: 0n,
      ops: [
        {
          type: "updateEntity",
          id: entityId,
          set: [
            {
              property: propId,
              value: { type: "text", value: "A", language: undefined },
            },
          ],
          unset: [{ property: propId, language: { type: "english" } }],
        },
      ],
    };

    expect(() => encodeEdit(edit)).toThrow("conflicts with set");
  });

  it("throws when updateRelation set and unset overlap", () => {
    const editId = randomId();
    const relationId = randomId();
    const spaceId = randomId();

    const edit: Edit = {
      id: editId,
      name: "Test Edit",
      authors: [],
      createdAt: 0n,
      ops: [
        {
          type: "updateRelation",
          id: relationId,
          fromSpace: spaceId,
          unset: ["fromSpace"],
        },
      ],
    };

    expect(() => encodeEdit(edit)).toThrow("unset contains field also set");
  });

  it("throws when unset language is used for non-text property", () => {
    const editId = randomId();
    const entityId = randomId();
    const propId = properties.population();

    const edit: Edit = {
      id: editId,
      name: "Test Edit",
      authors: [],
      createdAt: 0n,
      ops: [
        {
          type: "updateEntity",
          id: entityId,
          set: [
            {
              property: propId,
              value: { type: "int64", value: 1n, unit: undefined },
            },
          ],
          unset: [{ property: propId, language: { type: "english" } }],
        },
      ],
    };

    expect(() => encodeEdit(edit)).toThrow("unset language requires TEXT");
  });

  it("encodes and decodes all value types", () => {
    const editId = randomId();
    const entityId = randomId();

    const edit = new EditBuilder(editId)
      .setName("All Types Test")
      .createEntity(entityId, (e) =>
        e.bool(parseId("11111111111111111111111111111111")!, true)
         .int64(parseId("22222222222222222222222222222222")!, -42n, undefined)
         .float64(parseId("33333333333333333333333333333333")!, 3.14159, undefined)
         .text(parseId("44444444444444444444444444444444")!, "Hello World", undefined)
         .bytes(parseId("55555555555555555555555555555555")!, new Uint8Array([1, 2, 3, 4]))
         .schedule(parseId("66666666666666666666666666666666")!, "FREQ=WEEKLY;BYDAY=MO")
         .date(parseId("77777777777777777777777777777777")!, "2024-01-15Z")
         .point(parseId("88888888888888888888888888888888")!, 40.7128, -74.006)
      )
      .build();

    const encoded = encodeEdit(edit);
    const decoded = decodeEdit(encoded);

    expect(decoded.ops.length).toBe(1);
    const op = decoded.ops[0];
    expect(op.type).toBe("createEntity");

    if (op.type === "createEntity") {
      expect(op.values.length).toBe(8);

      // Check each value type
      const boolVal = op.values.find(v => v.value.type === "bool");
      expect(boolVal?.value).toEqual({ type: "bool", value: true });

      const intVal = op.values.find(v => v.value.type === "int64");
      expect(intVal?.value).toEqual({ type: "int64", value: -42n, unit: undefined });

      const floatVal = op.values.find(v => v.value.type === "float64");
      if (floatVal?.value.type === "float64") {
        expect(floatVal.value.value).toBeCloseTo(3.14159, 5);
      }

      const textVal = op.values.find(v => v.value.type === "text");
      expect(textVal?.value).toEqual({ type: "text", value: "Hello World", language: undefined });

      const pointVal = op.values.find(v => v.value.type === "point");
      if (pointVal?.value.type === "point") {
        expect(pointVal.value.lon).toBeCloseTo(-74.006, 3);
        expect(pointVal.value.lat).toBeCloseTo(40.7128, 4);
      }
    }
  });

  it("encodes and decodes RFC 3339 date/time/datetime values", () => {
    const editId = randomId();
    const entityId = randomId();

    const edit = new EditBuilder(editId)
      .setName("DateTime Test")
      .createEntity(entityId, (e) =>
        e.date(parseId("11111111111111111111111111111111")!, "2024-01-15Z")
         .date(parseId("22222222222222222222222222222222")!, "2024-06-30+05:30")
         .time(parseId("33333333333333333333333333333333")!, "14:30:45.123456Z")
         .time(parseId("44444444444444444444444444444444")!, "10:15:00-08:00")
         .datetime(parseId("55555555555555555555555555555555")!, "2024-01-15T14:30:45.123456Z")
         .datetime(parseId("66666666666666666666666666666666")!, "2024-12-31T23:59:59+05:30")
      )
      .build();

    const encoded = encodeEdit(edit);
    const decoded = decodeEdit(encoded);

    expect(decoded.ops.length).toBe(1);
    const op = decoded.ops[0];
    expect(op.type).toBe("createEntity");

    if (op.type === "createEntity") {
      expect(op.values.length).toBe(6);

      // Check date values
      const dateVal1 = op.values.find(v =>
        v.value.type === "date" && v.value.value === "2024-01-15Z");
      expect(dateVal1).toBeDefined();

      const dateVal2 = op.values.find(v =>
        v.value.type === "date" && v.value.value === "2024-06-30+05:30");
      expect(dateVal2).toBeDefined();

      // Check time values
      const timeVal1 = op.values.find(v =>
        v.value.type === "time" && v.value.value === "14:30:45.123456Z");
      expect(timeVal1).toBeDefined();

      const timeVal2 = op.values.find(v =>
        v.value.type === "time" && v.value.value === "10:15:00-08:00");
      expect(timeVal2).toBeDefined();

      // Check datetime values
      const dtVal1 = op.values.find(v =>
        v.value.type === "datetime" && v.value.value === "2024-01-15T14:30:45.123456Z");
      expect(dtVal1).toBeDefined();

      const dtVal2 = op.values.find(v =>
        v.value.type === "datetime" && v.value.value === "2024-12-31T23:59:59+05:30");
      expect(dtVal2).toBeDefined();
    }
  });

  it("encodes and decodes relations", () => {
    const editId = randomId();
    const from = randomId();
    const to = randomId();
    const relId = randomId();

    const edit = new EditBuilder(editId)
      .createEmptyEntity(from)
      .createEmptyEntity(to)
      .createRelationSimple(relId, from, to, relationTypes.types())
      .build();

    const encoded = encodeEdit(edit);
    const decoded = decodeEdit(encoded);

    expect(decoded.ops.length).toBe(3);
    const rel = decoded.ops[2];
    expect(rel.type).toBe("createRelation");

    if (rel.type === "createRelation") {
      expect(idsEqual(rel.id, relId)).toBe(true);
      expect(idsEqual(rel.from, from)).toBe(true);
      expect(idsEqual(rel.to, to)).toBe(true);
    }
  });

  it("encodes and decodes update/delete/restore operations", () => {
    const editId = randomId();
    const entityId = randomId();
    const relationId = randomId();
    const propId = randomId();

    const edit = new EditBuilder(editId)
      .updateEntity(entityId, (u) =>
        u.setText(propId, "Updated", undefined)
      )
      .deleteEntity(entityId)
      .restoreEntity(entityId)
      .updateRelation(relationId, (u) => u.setPosition("abc"))
      .deleteRelation(relationId)
      .restoreRelation(relationId)
      .build();

    const encoded = encodeEdit(edit);
    const decoded = decodeEdit(encoded);

    expect(decoded.ops.length).toBe(6);
    expect(decoded.ops[0].type).toBe("updateEntity");
    expect(decoded.ops[1].type).toBe("deleteEntity");
    expect(decoded.ops[2].type).toBe("restoreEntity");
    expect(decoded.ops[3].type).toBe("updateRelation");
    expect(decoded.ops[4].type).toBe("deleteRelation");
    expect(decoded.ops[5].type).toBe("restoreRelation");

    const updateRel = decoded.ops[3];
    if (updateRel.type === "updateRelation") {
      expect(updateRel.position).toBe("abc");
    }
  });

  it("canonical encoding is deterministic", () => {
    const editId = parseId("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")!;
    const entityId = parseId("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")!;

    const edit = new EditBuilder(editId)
      .setName("Canonical Test")
      .setCreatedAt(1000000n)
      .createEntity(entityId, (e) =>
        e.text(properties.name(), "Test", undefined)
      )
      .build();

    const encoded1 = encodeEdit(edit, { canonical: true });
    const encoded2 = encodeEdit(edit, { canonical: true });

    expect(encoded1.length).toBe(encoded2.length);
    for (let i = 0; i < encoded1.length; i++) {
      expect(encoded1[i]).toBe(encoded2[i]);
    }
  });
});

describe("Compression", () => {
  it("isCompressed detects GRC2Z magic", () => {
    const compressed = new Uint8Array([0x47, 0x52, 0x43, 0x32, 0x5a, 0x00]); // "GRC2Z" + data
    const uncompressed = new Uint8Array([0x47, 0x52, 0x43, 0x32, 0x00]); // "GRC2" + data

    expect(isCompressed(compressed)).toBe(true);
    expect(isCompressed(uncompressed)).toBe(false);
  });

  it("isCompressed returns false for short data", () => {
    expect(isCompressed(new Uint8Array([0x47, 0x52, 0x43, 0x32]))).toBe(false);
    expect(isCompressed(new Uint8Array([]))).toBe(false);
  });

  it("encodes and decodes compressed edit", async () => {
    const editId = randomId();
    const entityId = randomId();

    const edit = new EditBuilder(editId)
      .setName("Compressed Test")
      .setCreatedAt(1234567890000000n)
      .createEntity(entityId, (e) =>
        e.text(properties.name(), "Alice", undefined)
         .text(properties.description(), "A person named Alice with a long description to make compression worthwhile", undefined)
      )
      .build();

    const compressed = await encodeEditCompressed(edit);

    // Check magic bytes
    expect(String.fromCharCode(...compressed.slice(0, 5))).toBe("GRC2Z");

    // Verify it's detected as compressed
    expect(isCompressed(compressed)).toBe(true);

    // Decode and verify
    const decoded = await decodeEditCompressed(compressed);

    expect(idsEqual(decoded.id, edit.id)).toBe(true);
    expect(decoded.name).toBe(edit.name);
    expect(decoded.createdAt).toBe(edit.createdAt);
    expect(decoded.ops.length).toBe(edit.ops.length);
  });

  it("compressed data is smaller than uncompressed for larger edits", async () => {
    const editId = randomId();

    // Create an edit with repetitive data (good for compression)
    const builder = new EditBuilder(editId).setName("Large Test");

    for (let i = 0; i < 50; i++) {
      const entityId = randomId();
      builder.createEntity(entityId, (e) =>
        e.text(properties.name(), `Entity number ${i} with some padding text`, undefined)
         .text(properties.description(), "This is a repeated description that should compress well", undefined)
      );
    }

    const edit = builder.build();

    const uncompressed = encodeEdit(edit);
    const compressed = await encodeEditCompressed(edit);

    // Compressed should be smaller
    expect(compressed.length).toBeLessThan(uncompressed.length);
  });

  it("decodeEditAuto handles both formats", async () => {
    const editId = randomId();
    const entityId = randomId();

    const edit = new EditBuilder(editId)
      .setName("Auto Test")
      .createEntity(entityId, (e) =>
        e.text(properties.name(), "Test", undefined)
      )
      .build();

    const uncompressed = encodeEdit(edit);
    const compressed = await encodeEditCompressed(edit);

    // Should decode both formats
    const decoded1 = await decodeEditAuto(uncompressed);
    const decoded2 = await decodeEditAuto(compressed);

    expect(idsEqual(decoded1.id, edit.id)).toBe(true);
    expect(idsEqual(decoded2.id, edit.id)).toBe(true);
    expect(decoded1.name).toBe(edit.name);
    expect(decoded2.name).toBe(edit.name);
  });

  it("compressed canonical encoding roundtrips", async () => {
    const editId = parseId("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")!;
    const entityId = parseId("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")!;

    const edit = new EditBuilder(editId)
      .setName("Canonical Compressed Test")
      .setCreatedAt(1000000n)
      .createEntity(entityId, (e) =>
        e.text(properties.name(), "Test", undefined)
      )
      .build();

    const compressed = await encodeEditCompressed(edit, { canonical: true });
    const decoded = await decodeEditCompressed(compressed);

    expect(idsEqual(decoded.id, edit.id)).toBe(true);
    expect(decoded.name).toBe(edit.name);
  });

  it("preloadCompression loads WASM", async () => {
    // After preloading, compression should be ready
    await preloadCompression();
    expect(isCompressionReady()).toBe(true);
  });

  it("encodeEditAuto returns uncompressed for small edits", async () => {
    const editId = randomId();
    const entityId = randomId();

    // Create a small edit
    const edit = new EditBuilder(editId)
      .setName("Small")
      .createEmptyEntity(entityId)
      .build();

    // With default threshold, small edits should not be compressed
    const encoded = await encodeEditAuto(edit);

    // Should have GRC2 magic (uncompressed)
    expect(String.fromCharCode(...encoded.slice(0, 4))).toBe("GRC2");
    expect(isCompressed(encoded)).toBe(false);

    // Should decode correctly
    const decoded = await decodeEditAuto(encoded);
    expect(idsEqual(decoded.id, edit.id)).toBe(true);
  });

  it("encodeEditAuto compresses large edits", async () => {
    const editId = randomId();

    // Create a large edit
    const builder = new EditBuilder(editId).setName("Large Auto Test");
    for (let i = 0; i < 20; i++) {
      const entityId = randomId();
      builder.createEntity(entityId, (e) =>
        e.text(properties.name(), `Entity ${i} with padding`, undefined)
         .text(properties.description(), "Repeated description for compression", undefined)
      );
    }
    const edit = builder.build();

    // Should be compressed (above default threshold)
    const encoded = await encodeEditAuto(edit);
    expect(isCompressed(encoded)).toBe(true);

    // Should decode correctly
    const decoded = await decodeEditAuto(encoded);
    expect(idsEqual(decoded.id, edit.id)).toBe(true);
    expect(decoded.ops.length).toBe(edit.ops.length);
  });

  it("encodeEditAuto respects threshold option", async () => {
    const editId = randomId();
    const entityId = randomId();

    const edit = new EditBuilder(editId)
      .setName("Threshold Test")
      .createEntity(entityId, (e) =>
        e.text(properties.name(), "Test", undefined)
      )
      .build();

    // With threshold: 0, should always compress
    const alwaysCompressed = await encodeEditAuto(edit, { threshold: 0 });
    expect(isCompressed(alwaysCompressed)).toBe(true);

    // With threshold: Infinity, should never compress
    const neverCompressed = await encodeEditAuto(edit, { threshold: Infinity });
    expect(isCompressed(neverCompressed)).toBe(false);

    // Both should decode correctly
    const decoded1 = await decodeEditAuto(alwaysCompressed);
    const decoded2 = await decodeEditAuto(neverCompressed);
    expect(idsEqual(decoded1.id, edit.id)).toBe(true);
    expect(idsEqual(decoded2.id, edit.id)).toBe(true);
  });
});
