<?hh

<<file:__EnableUnstableFeatures('expression_trees')>>

class Foo {
  public ?int $x;
  public function reset(): int {
    return 1;
  }
}

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

  public function lambdaLiteral(
    vec<string> $_args,
    vec<this::TAst> $_body,
  ): this::TAst {
    throw new Exception();
  }

  public function splice(
    mixed $_,
  ): this::TAst {
    throw new Exception();
  }

  // TODO: it would be better to discard unsupported syntax nodes during lowering.
  public function unsupportedSyntax(string $msg): this::TAst {
    throw new Exception($msg);
  }
}

function test(): void {
  $x = new Foo();

  if ($x->x !== null) {
    $_ = Code`() ==> {
      __splice__($x->reset()) +
      // We should think that $x->x could be null
      __splice__($x->x + 1);
      return;
    }`;
  }
}
