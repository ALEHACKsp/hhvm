<?hh

<<file:__EnableUnstableFeatures('expression_trees')>>

// Placeholder definition so we don't get naming/typing errors.
class Code {
  const type TAst = mixed;
  // Simple literals.
  public function intLiteral(int $_): this::TAst {
    throw new Exception();
  }
  public function boolLiteral(bool $_): this::TAst {
    throw new Exception();
  }
  public function stringLiteral(string $_): this::TAst {
    throw new Exception();
  }
  public function nullLiteral(): this::TAst {
    throw new Exception();
  }
  public function localVar(string $_): this::TAst {
    throw new Exception();
  }

  // Operators
  public function plus(this::TAst $_, this::TAst $_): this::TAst {
    throw new Exception();
  }
  public function call(string $_fnName, vec<this::TAst> $_args): this::TAst {
    throw new Exception();
  }
  public function exclamationMark(this::TAst $_): this::TAst {
    throw new Exception();
  }
  public function ampamp(this::TAst $_, this::TAst $_): this::TAst {
    throw new Exception();
  }
  public function barbar(this::TAst $_, this::TAst $_): this::TAst {
    throw new Exception();
  }

  // Statements.
  public function assign(this::TAst $_, this::TAst $_): this::TAst {
    throw new Exception();
  }
  public function ifStatement(
    this::TAst $_cond,
    vec<this::TAst> $_then_body,
    vec<this::TAst> $_else_body,
  ): this::TAst {
    throw new Exception();
  }
  public function whileStatement(
    this::TAst $_cond,
    vec<this::TAst> $_body,
  ): this::TAst {
    throw new Exception();
  }
  public function returnStatement(?this::TAst $_): this::TAst {
    throw new Exception();
  }
  public function forStatement(
    vec<this::TAst> $_init,
    ?this::TAst $_cond,
    vec<this::TAst> $_incr,
    vec<this::TAst> $_body,
  ): this::TAst {
    throw new Exception();
  }

  public function lambdaLiteral(
    vec<string> $_args,
    vec<this::TAst> $_body,
  ): this::TAst {
    throw new Exception();
  }
}

final class ExprTree<TVisitor, TResult, TInfer>{
  public function __construct(
    private (function(TVisitor): TResult) $x,
    private (function(): TInfer) $err,
  ) {}
}

function nullable_bool(): ?bool { return null; }
function a_bool(): bool { return true; }

/**
 * Since all Hack types are truthy, typically, most syntactic places that
 * expect booleans allow all types. However, as to not leak these truthy
 * Hack semantics to Expression Trees, ensure that those syntactic positions
 * only accept booleans, rather than any truthy expression.
 */
function test(): void {
  $y = Code`
    () ==> {
      // if/else
      if (nullable_bool()) {}
      if (nullable_bool()) {} else {}

      if (a_bool()) {}
      if (a_bool()) {} else {}

      if (a_bool()) {}
      else if (nullable_bool()) {}

      if (a_bool()) {}
      else if (nullable_bool()) {}
      else {}

      if (a_bool()) {}
      else if (a_bool()) {}
      else {}

      // while() {}
      while(nullable_bool()) {}
      while(a_bool()) {}

      // for (;;) {}
      for (;;) {}

      for (;nullable_bool();) {}
      for ($i = 0; nullable_bool();) {}
      for (; nullable_bool(); $i = $i + 1) {}
      for ($i = 0; nullable_bool(); $i = $i + 1) {}

      for (;a_bool();) {}
      for ($i = 0; a_bool();) {}
      for (; a_bool(); $i = $i + 1) {}
      for ($i = 0; a_bool(); $i = $i + 1) {}

      // Boolean ||
      nullable_bool() || a_bool();
      a_bool() || nullable_bool();
      a_bool() || a_bool();

      // Boolean &&
      nullable_bool() && a_bool();
      a_bool() && nullable_bool();
      a_bool() && a_bool();

      // Boolean !
      !nullable_bool();
      !a_bool();
    }
  `;
}
