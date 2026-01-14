import type { Id } from "../types/id.js";
import type { Edit } from "../types/edit.js";
import type {
  CreateRelation,
  Op,
} from "../types/op.js";
import { DataType } from "../types/value.js";
import { EntityBuilder } from "./entity.js";
import { UpdateEntityBuilder } from "./update.js";
import { RelationBuilder } from "./relation.js";
import { UpdateRelationBuilder } from "./update-relation.js";

/**
 * Builder for constructing an Edit with operations.
 */
export class EditBuilder {
  private readonly _id: Id;
  private name: string = "";
  private authors: Id[] = [];
  private createdAt: bigint = 0n;
  private ops: Op[] = [];

  constructor(id: Id) {
    this._id = id;
  }

  /**
   * Sets the edit name.
   */
  setName(name: string): this {
    this.name = name;
    return this;
  }

  /**
   * Adds an author to the edit.
   */
  addAuthor(authorId: Id): this {
    this.authors.push(authorId);
    return this;
  }

  /**
   * Sets multiple authors at once.
   */
  setAuthors(authorIds: Id[]): this {
    this.authors = [...authorIds];
    return this;
  }

  /**
   * Sets the creation timestamp (microseconds since Unix epoch).
   */
  setCreatedAt(timestamp: bigint): this {
    this.createdAt = timestamp;
    return this;
  }

  /**
   * Sets the creation timestamp to now.
   */
  setCreatedNow(): this {
    this.createdAt = BigInt(Date.now()) * 1000n;
    return this;
  }

  // =========================================================================
  // Property Operations
  // =========================================================================

  /**
   * Adds a CreateProperty operation.
   */
  createProperty(id: Id, dataType: DataType): this {
    this.ops.push({ type: "createProperty", id, dataType });
    return this;
  }

  // =========================================================================
  // Entity Operations
  // =========================================================================

  /**
   * Adds a CreateEntity operation using a builder function.
   */
  createEntity(id: Id, build: (b: EntityBuilder) => EntityBuilder): this {
    const builder = build(new EntityBuilder());
    this.ops.push({
      type: "createEntity",
      id,
      values: builder.getValues(),
    });
    return this;
  }

  /**
   * Adds a CreateEntity operation with no values.
   */
  createEmptyEntity(id: Id): this {
    this.ops.push({
      type: "createEntity",
      id,
      values: [],
    });
    return this;
  }

  /**
   * Adds an UpdateEntity operation using a builder function.
   */
  updateEntity(id: Id, build: (b: UpdateEntityBuilder) => UpdateEntityBuilder): this {
    const builder = build(new UpdateEntityBuilder(id));
    this.ops.push({
      type: "updateEntity",
      id: builder.id,
      set: builder.getSet(),
      unset: builder.getUnset(),
    });
    return this;
  }

  /**
   * Adds a DeleteEntity operation.
   */
  deleteEntity(id: Id): this {
    this.ops.push({ type: "deleteEntity", id });
    return this;
  }

  /**
   * Adds a RestoreEntity operation.
   */
  restoreEntity(id: Id): this {
    this.ops.push({ type: "restoreEntity", id });
    return this;
  }

  // =========================================================================
  // Relation Operations
  // =========================================================================

  /**
   * Adds a simple CreateRelation operation with explicit ID.
   */
  createRelationSimple(id: Id, from: Id, to: Id, relationType: Id): this {
    this.ops.push({
      type: "createRelation",
      id,
      relationType,
      from,
      to,
    });
    return this;
  }

  /**
   * Adds a CreateRelation operation with full control using a builder.
   */
  createRelation(build: (b: RelationBuilder) => RelationBuilder): this {
    const builder = build(new RelationBuilder());
    const relation = builder.build();
    if (relation) {
      this.ops.push(relation);
    }
    return this;
  }

  /**
   * Adds a CreateRelation directly.
   */
  addRelation(relation: CreateRelation): this {
    this.ops.push(relation);
    return this;
  }

  /**
   * Adds an UpdateRelation operation using a builder function.
   */
  updateRelation(id: Id, build: (b: UpdateRelationBuilder) => UpdateRelationBuilder): this {
    const builder = build(new UpdateRelationBuilder(id));
    this.ops.push(builder.build());
    return this;
  }

  /**
   * Adds a DeleteRelation operation.
   */
  deleteRelation(id: Id): this {
    this.ops.push({ type: "deleteRelation", id });
    return this;
  }

  /**
   * Adds a RestoreRelation operation.
   */
  restoreRelation(id: Id): this {
    this.ops.push({ type: "restoreRelation", id });
    return this;
  }

  // =========================================================================
  // Raw Operations
  // =========================================================================

  /**
   * Adds a raw operation directly.
   */
  addOp(op: Op): this {
    this.ops.push(op);
    return this;
  }

  /**
   * Adds multiple raw operations.
   */
  addOps(ops: Op[]): this {
    this.ops.push(...ops);
    return this;
  }

  // =========================================================================
  // Build
  // =========================================================================

  /**
   * Returns the number of operations added so far.
   */
  opCount(): number {
    return this.ops.length;
  }

  /**
   * Builds the final Edit.
   */
  build(): Edit {
    return {
      id: this._id,
      name: this.name,
      authors: this.authors,
      createdAt: this.createdAt,
      ops: this.ops,
    };
  }
}
