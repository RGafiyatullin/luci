
# Variables / Bindings

* Keys start with `$` (e.g. `$ACCOUNT_ID`). `$_` is a reserved name.
* Binds happen from literals or previously bound values.
* No overwrite: rebinding must equal existing value or it won't match.

## `src` (source)

* The value to read and compare.
* May be:
  - A literal.
  - A bound variable.
* Using an unbound variable in `src` is a fatal error: the scenario fails immediately.
* When `src` is a bound variable, its current value is used as-is.
* Literals are compared directly, without type coercion.

## `dst` (destination)
* The variable or literal location to bind or compare against.
* May be:
  - An unbound variable -> binds the `src` value.
  - A bound variable -> compares against `src` value; must be equal to complete.
  - A literal -> compares directly; must be equal to complete.
* If comparison fails, the node stays incomplete forever (it never re-attempts).

## Matching semantics
* Objects: partial match allowed; extra fields in actual data are ignored.
* Arrays: positional match; `$VAR` binds a position, `$_` ignores it.
* Bound values never change — all scopes are write-once for each variable.

## Scopes
* Each subroutine invocation has its own scope.
* Variables from caller to callee are passed explicitly via `in` (`src` -> `dst`).
* Variables from callee to caller are passed explicitly via `out` (`src` -> `dst`) and **only on success**.
* Scopes are isolated: same variable name in caller and callee refer to different bindings unless passed.

## Examples

Bind on first use:
```yaml
dst: $USER_ID
src: 42          # $USER_ID becomes 42
```

Check equality against existing bind:
```yaml
dst: $USER_ID    # already bound to 42
src: 42          # matches — complete
```

Mismatch — stays incomplete:
```yaml
dst: $USER_ID    # already bound to 42
src: 99          # mismatch — never completes
```

Unbound source — fatal error:
```yaml
dst: $OTHER_ID
src: $USER_ID    # if $USER_ID not bound → fail scenario
```

Ignore the value:
```yaml
dst: $_
src: 99          # no binding stored
```
