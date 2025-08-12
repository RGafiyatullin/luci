
# Variables / Bindings

## Naming

- Keys start with `$` (e.g. `$USER_ID`).
- `$_` is a reserved destination — values assigned to it are discarded and never stored (a `/dev/null`).
- Variables can be scalars, arrays, or objects.
- Variables are immutable within a scope — once bound, their value cannot change.

## Constructing values (`src`)

- `src` provides the value to compare against or to assign into `dst`.
- Can be:
  - A literal (string, number, boolean, null).
  - A bound variable (must already have a value).
  - A composite structure (array or object) built from literals and bound variables.
- Using an unbound variable in `src` is a fatal error — scenario stops immediately (this can probably be statically checked, but this has not yet been implemented).

### Example — construct an object from bound vars:

```yaml
dst: $USER_INFO # assuming previously unbound
src:
  id: $USER_ID
  roles: [ "admin", $ROLE ]
```

### Example — construct an array:

```yaml
dst: $NAMES # assuming previously unbound
src: [ $FIRST, "middle", $LAST ]
```

## Deconstructing values (`dst`)

`dst` specifies where values from src go and how they are checked.

- Can be:
  - An unbound variable — binds the `src` value.
  - A bound variable — must equal the `src` value.
  - A literal — must equal the `src` value.
  - A composite structure (array or object) with variables/literals in positions or fields.
- It is not an error if a bound variable or literal in `dst` doesn't match `src` — it just not a match.

### Example — deconstruct object:
```yaml
dst:
  id: $USER_ID
  meta:
    version: $VER
    info: $_
src:
  id: 123
  meta:
    version: 1.2
    info: nothing in particular
    a_new_field: something no one cares about
```

### Example — deconstruct array:
```yaml
dst: [ $HEAD, $_, $TAIL ]
src: [ "my car", "is", "cdr (well, not exactly)" ]
```

## Matching

- Scalars: strict equality, no type coercion.
- Objects: partial match allowed — extra fields in the actual data are ignored; nested structures match recursively.
- Arrays: positional match — `$VAR` binds, `$_` ignores, nested arrays/objects match recursively.

## Scopes

- Each subroutine invocation has its own isolated variable scope.
- Variables are passed explicitly between caller and callee:
  - `in`: caller — callee at call start.
  - `out`: callee — caller on successful completion only.
- Same variable name in different scopes refers to different bindings unless explicitly passed.

## Examples

### Bind scalar on first use:

```yaml
dst: $USER_ID
src: 42
```

### Match against existing value:

```yaml
dst: $USER_ID    # already bound to 42
src: 42
```

### Construct composite in `src`, deconstruct in `dst`:

```yaml
dst:
  user: $USER
  tags: [ $TAG1, $_ ]
src:
  user: "alice"
  tags: [ "x", "y" ]
```

Binds `$USER="alice"`, `$TAG1="x"`.


### Partial object match:

```yaml
dst:
  id: $ID
src:
  id: 7
  extra: "ignored"
```

### Array with nested match:

```yaml
dst:
  matrix: [ [ $A, $_ ], $_ ]
src:
  matrix: [ [ 1, 2 ], [ 3, 4 ] ]
```

Binds `$A=1`.

### Ignore value entirely:

```yaml
dst: $_
src: 99
```

