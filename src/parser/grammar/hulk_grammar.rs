use super::Grammar;
use super::production::Production;
use super::symbol::{NonTerminal, Terminal, Symbol};

// Alias cortos para no repetir ruido sintáctico
fn nt(n: NonTerminal) -> Symbol { Symbol::NT(n) }
fn t(t: Terminal)     -> Symbol { Symbol::T(t)  }
fn p(head: NonTerminal, body: Vec<Symbol>) -> Production { Production::new(head, body) }
fn eps(head: NonTerminal)                  -> Production { Production::epsilon(head)   }

/// Construye y devuelve la gramática completa de HULK.
///
/// Las producciones están ordenadas de arriba a abajo en el mismo orden
/// en que se asignan los ids (0, 1, 2, …).  Ese orden es relevante para
/// la resolución de conflictos shift/reduce (se prefiere la producción de
/// menor id cuando hay ambigüedad).
pub fn build() -> Grammar {
    use NonTerminal::*;
    use Terminal::*;

    let mut g = Grammar::new(Start);

    // ═══════════════════════════════════════════════════════════════════
    //  0.  Producción aumentada  S' → Program $
    // ═══════════════════════════════════════════════════════════════════
    g.add(p(Start, vec![nt(Program), t(Eof)]));

    // ═══════════════════════════════════════════════════════════════════
    //  1.  Programa
    //      Program → DeclList Expr
    //              | DeclList Expr ;
    //
    //  Un programa HULK es una lista de declaraciones seguida de una
    //  expresión global (el punto de entrada).  El `;` final es opcional.
    // ═══════════════════════════════════════════════════════════════════
    g.add(p(Program, vec![nt(DeclList), nt(Expr)]));
    g.add(p(Program, vec![nt(DeclList), nt(Expr), t(Semicolon)]));

    // ═══════════════════════════════════════════════════════════════════
    //  2.  Lista de declaraciones (puede ser vacía)
    // ═══════════════════════════════════════════════════════════════════
    g.add(eps(DeclList));
    g.add(p(DeclList, vec![nt(DeclList), nt(Decl)]));

    // ═══════════════════════════════════════════════════════════════════
    //  3.  Declaración
    // ═══════════════════════════════════════════════════════════════════
    g.add(p(Decl, vec![nt(FuncDecl)]));
    g.add(p(Decl, vec![nt(TypeDecl)]));
    g.add(p(Decl, vec![nt(ProtocolDecl)]));

    // ═══════════════════════════════════════════════════════════════════
    //  4.  Declaración de función
    //
    //  Forma inline:   function f(params) : RetType => expr ;
    //  Forma completa: function f(params) : RetType { block }
    //  La anotación de tipo de retorno es opcional.
    // ═══════════════════════════════════════════════════════════════════

    // Sin anotación de retorno
    g.add(p(FuncDecl, vec![
        t(Function), t(Identifier),
        t(LParen), nt(ParamList), t(RParen),
        t(Arrow), nt(Expr), t(Semicolon),
    ]));
    g.add(p(FuncDecl, vec![
        t(Function), t(Identifier),
        t(LParen), nt(ParamList), t(RParen),
        nt(BlockExpr),
    ]));

    // Con anotación de retorno
    g.add(p(FuncDecl, vec![
        t(Function), t(Identifier),
        t(LParen), nt(ParamList), t(RParen),
        t(Colon), nt(TypeName),
        t(Arrow), nt(Expr), t(Semicolon),
    ]));
    g.add(p(FuncDecl, vec![
        t(Function), t(Identifier),
        t(LParen), nt(ParamList), t(RParen),
        t(Colon), nt(TypeName),
        nt(BlockExpr),
    ]));

    // ═══════════════════════════════════════════════════════════════════
    //  5.  Parámetros
    // ═══════════════════════════════════════════════════════════════════
    g.add(eps(ParamList));
    g.add(p(ParamList, vec![nt(ParamListNonEmpty)]));

    g.add(p(ParamListNonEmpty, vec![nt(Param)]));
    g.add(p(ParamListNonEmpty, vec![nt(ParamListNonEmpty), t(Comma), nt(Param)]));

    // Param → id  |  id : TypeName
    g.add(p(Param, vec![t(Identifier)]));
    g.add(p(Param, vec![t(Identifier), t(Colon), nt(TypeName)]));

    // ═══════════════════════════════════════════════════════════════════
    //  6.  Declaración de tipo
    //
    //  type Name TypeArgs InheritClause { TypeBody }
    //
    //  TypeArgs     → ε  |  ( TypeArgList )
    //  InheritClause→ ε  |  inherits Name  |  inherits Name(ArgList)
    // ═══════════════════════════════════════════════════════════════════
    g.add(p(TypeDecl, vec![
        t(Type), t(Identifier),
        nt(TypeArgs), nt(InheritClause),
        t(LBrace), nt(TypeBody), t(RBrace),
    ]));

    g.add(eps(TypeArgs));
    g.add(p(TypeArgs, vec![t(LParen), nt(TypeArgList), t(RParen)]));

    g.add(p(TypeArgList, vec![nt(Param)]));
    g.add(p(TypeArgList, vec![nt(TypeArgList), t(Comma), nt(Param)]));

    g.add(eps(InheritClause));
    g.add(p(InheritClause, vec![t(Inherits), nt(TypeName)]));
    g.add(p(InheritClause, vec![
        t(Inherits), nt(TypeName),
        t(LParen), nt(ArgList), t(RParen),
    ]));

    // ── Cuerpo del tipo ─────────────────────────────────────────────────
    g.add(p(TypeBody, vec![nt(TypeMemberList)]));

    g.add(eps(TypeMemberList));
    g.add(p(TypeMemberList, vec![nt(TypeMemberList), nt(TypeMember)]));

    g.add(p(TypeMember, vec![nt(AttributeDef)]));
    g.add(p(TypeMember, vec![nt(MethodDef)]));

    // AttributeDef → id = Expr ;
    //              | id : TypeName = Expr ;
    g.add(p(AttributeDef, vec![
        t(Identifier), t(Assign), nt(Expr), t(Semicolon),
    ]));
    g.add(p(AttributeDef, vec![
        t(Identifier), t(Colon), nt(TypeName),
        t(Assign), nt(Expr), t(Semicolon),
    ]));

    // MethodDef — igual que FuncDecl pero sin la palabra `function`
    g.add(p(MethodDef, vec![
        t(Identifier),
        t(LParen), nt(ParamList), t(RParen),
        t(Arrow), nt(Expr), t(Semicolon),
    ]));
    g.add(p(MethodDef, vec![
        t(Identifier),
        t(LParen), nt(ParamList), t(RParen),
        nt(BlockExpr),
    ]));
    g.add(p(MethodDef, vec![
        t(Identifier),
        t(LParen), nt(ParamList), t(RParen),
        t(Colon), nt(TypeName),
        t(Arrow), nt(Expr), t(Semicolon),
    ]));
    g.add(p(MethodDef, vec![
        t(Identifier),
        t(LParen), nt(ParamList), t(RParen),
        t(Colon), nt(TypeName),
        nt(BlockExpr),
    ]));

    // ═══════════════════════════════════════════════════════════════════
    //  7.  Declaración de protocolo
    //
    //  protocol Name { ProtocolBody }
    //  protocol Name extends Name { ProtocolBody }
    // ═══════════════════════════════════════════════════════════════════
    g.add(p(ProtocolDecl, vec![
        t(Protocol), t(Identifier),
        t(LBrace), nt(ProtocolBody), t(RBrace),
    ]));
    g.add(p(ProtocolDecl, vec![
        t(Protocol), t(Identifier),
        t(Extends), nt(TypeName),
        t(LBrace), nt(ProtocolBody), t(RBrace),
    ]));

    g.add(p(ProtocolBody, vec![nt(ProtocolMemberList)]));

    g.add(eps(ProtocolMemberList));
    g.add(p(ProtocolMemberList, vec![nt(ProtocolMemberList), nt(ProtocolMember)]));

    // ProtocolMember → MethodSignature ;
    g.add(p(ProtocolMember, vec![nt(MethodSignature), t(Semicolon)]));

    // MethodSignature → id ( ParamList ) : TypeName
    g.add(p(MethodSignature, vec![
        t(Identifier),
        t(LParen), nt(ParamList), t(RParen),
        t(Colon), nt(TypeName),
    ]));

    // ═══════════════════════════════════════════════════════════════════
    //  8.  TypeName
    //
    //  HULK admite tres formas de nombre de tipo:
    //    Number      → tipo simple
    //    Number[]    → tipo vector
    //    Number*     → tipo iterable (usado en anotaciones de parámetros)
    //
    //  NOTA: en `new TypeName(…)` solo aplica la forma simple.
    //  La distinción es semántica; la gramática acepta las tres en todos
    //  los contextos y el análisis semántico restringe su uso.
    // ═══════════════════════════════════════════════════════════════════
    g.add(p(TypeName, vec![t(Identifier)]));
    g.add(p(TypeName, vec![t(Identifier), t(LBracket), t(RBracket)]));
    g.add(p(TypeName, vec![t(Identifier), t(Multiply)]));

    // ═══════════════════════════════════════════════════════════════════
    //  9.  Argumentos de llamada
    // ═══════════════════════════════════════════════════════════════════
    g.add(eps(ArgList));
    g.add(p(ArgList, vec![nt(ArgListNonEmpty)]));

    g.add(p(ArgListNonEmpty, vec![nt(Expr)]));
    g.add(p(ArgListNonEmpty, vec![nt(ArgListNonEmpty), t(Comma), nt(Expr)]));

    // ═══════════════════════════════════════════════════════════════════
    //  10. Jerarquía de expresiones  (precedencia de menor a mayor)
    //
    //  Expr
    //    AssignExpr    := += -= *= /= %=        (derecha-asociativo)
    //    OrExpr        |                         (izquierda)
    //    AndExpr       &                         (izquierda)
    //    CompareExpr   == != < > <= >=           (izquierda, no-encadenable)
    //    IsAsExpr      is  as                    (izquierda)
    //    ConcatExpr    @  @@                     (izquierda)
    //    AddExpr       +  -                      (izquierda)
    //    MulExpr       *  /  %                   (izquierda)
    //    PowerExpr     ^  **                     (DERECHA-asociativo)
    //    UnaryExpr     -  !                      (prefijo, derecha)
    //    PostfixExpr   ++  --                    (postfijo, izquierda)
    //    CallOrAccess  f()  .id  .id()  [i]      (izquierda)
    //    PrimaryExpr   literales, (expr), bloques, etc.
    // ═══════════════════════════════════════════════════════════════════

    // ── 10.1  Expr ──────────────────────────────────────────────────────
    g.add(p(Expr, vec![nt(AssignExpr)]));

    // ── 10.2  AssignExpr (derecha-asociativo por recursión derecha) ──────
    // Cualquier expresión puede aparecer a la izquierda de `:=` en la
    // gramática; la restricción a lvalues válidos es semántica.
    g.add(p(AssignExpr, vec![nt(OrExpr)]));
    g.add(p(AssignExpr, vec![nt(OrExpr), t(DestructAssign), nt(AssignExpr)]));
    g.add(p(AssignExpr, vec![nt(OrExpr), t(PlusAssign),     nt(AssignExpr)]));
    g.add(p(AssignExpr, vec![nt(OrExpr), t(MinusAssign),    nt(AssignExpr)]));
    g.add(p(AssignExpr, vec![nt(OrExpr), t(MultAssign),     nt(AssignExpr)]));
    g.add(p(AssignExpr, vec![nt(OrExpr), t(DivAssign),      nt(AssignExpr)]));
    g.add(p(AssignExpr, vec![nt(OrExpr), t(ModAssign),      nt(AssignExpr)]));

    // ── 10.3  OrExpr ────────────────────────────────────────────────────
    g.add(p(OrExpr, vec![nt(AndExpr)]));
    g.add(p(OrExpr, vec![nt(OrExpr), t(Or), nt(AndExpr)]));

    // ── 10.4  AndExpr ───────────────────────────────────────────────────
    g.add(p(AndExpr, vec![nt(CompareExpr)]));
    g.add(p(AndExpr, vec![nt(AndExpr), t(And), nt(CompareExpr)]));

    // ── 10.5  CompareExpr ───────────────────────────────────────────────
    // En HULK las comparaciones no se encadenan: `a < b < c` no es válido.
    // La gramática izquierdo-recursiva lo permite; la semántica lo prohíbe.
    g.add(p(CompareExpr, vec![nt(IsAsExpr)]));
    g.add(p(CompareExpr, vec![nt(CompareExpr), t(Equal),     nt(IsAsExpr)]));
    g.add(p(CompareExpr, vec![nt(CompareExpr), t(NotEqual),  nt(IsAsExpr)]));
    g.add(p(CompareExpr, vec![nt(CompareExpr), t(Less),      nt(IsAsExpr)]));
    g.add(p(CompareExpr, vec![nt(CompareExpr), t(Greater),   nt(IsAsExpr)]));
    g.add(p(CompareExpr, vec![nt(CompareExpr), t(LessEq),    nt(IsAsExpr)]));
    g.add(p(CompareExpr, vec![nt(CompareExpr), t(GreaterEq), nt(IsAsExpr)]));

    // ── 10.6  IsAsExpr ──────────────────────────────────────────────────
    g.add(p(IsAsExpr, vec![nt(ConcatExpr)]));
    g.add(p(IsAsExpr, vec![nt(IsAsExpr), t(Is), nt(TypeName)]));
    g.add(p(IsAsExpr, vec![nt(IsAsExpr), t(As), nt(TypeName)]));

    // ── 10.7  ConcatExpr ────────────────────────────────────────────────
    g.add(p(ConcatExpr, vec![nt(AddExpr)]));
    g.add(p(ConcatExpr, vec![nt(ConcatExpr), t(Concat),       nt(AddExpr)]));
    g.add(p(ConcatExpr, vec![nt(ConcatExpr), t(DoubleConcat), nt(AddExpr)]));

    // ── 10.8  AddExpr ───────────────────────────────────────────────────
    g.add(p(AddExpr, vec![nt(MulExpr)]));
    g.add(p(AddExpr, vec![nt(AddExpr), t(Plus),  nt(MulExpr)]));
    g.add(p(AddExpr, vec![nt(AddExpr), t(Minus), nt(MulExpr)]));

    // ── 10.9  MulExpr ───────────────────────────────────────────────────
    g.add(p(MulExpr, vec![nt(PowerExpr)]));
    g.add(p(MulExpr, vec![nt(MulExpr), t(Multiply), nt(PowerExpr)]));
    g.add(p(MulExpr, vec![nt(MulExpr), t(Divide),   nt(PowerExpr)]));
    g.add(p(MulExpr, vec![nt(MulExpr), t(Modulo),   nt(PowerExpr)]));

    // ── 10.10 PowerExpr (DERECHA-asociativo) ────────────────────────────
    // `a ^ b ^ c`  →  `a ^ (b ^ c)`
    // Ambos `^` y `**` son equivalentes en HULK.
    g.add(p(PowerExpr, vec![nt(UnaryExpr)]));
    g.add(p(PowerExpr, vec![nt(UnaryExpr), t(PowerCaret), nt(PowerExpr)]));
    g.add(p(PowerExpr, vec![nt(UnaryExpr), t(PowerStar),  nt(PowerExpr)]));

    // ── 10.11 UnaryExpr ─────────────────────────────────────────────────
    // `-` y `!` son prefijos con precedencia alta (más que los binarios).
    // Recursivos a la derecha para soportar `--x` y `!!x`.
    g.add(p(UnaryExpr, vec![nt(PostfixExpr)]));
    g.add(p(UnaryExpr, vec![t(Minus), nt(UnaryExpr)]));
    g.add(p(UnaryExpr, vec![t(Not),   nt(UnaryExpr)]));

    // ── 10.12 PostfixExpr ───────────────────────────────────────────────
    g.add(p(PostfixExpr, vec![nt(CallOrAccess)]));
    g.add(p(PostfixExpr, vec![nt(CallOrAccess), t(Increment)]));
    g.add(p(PostfixExpr, vec![nt(CallOrAccess), t(Decrement)]));

    // ── 10.13 CallOrAccess ──────────────────────────────────────────────
    // Todas las formas de acceso se encadenan con recursión izquierda:
    //   f(a)(b)       → doble llamada
    //   obj.method()  → llamada a método
    //   v[i][j]       → doble indexación
    g.add(p(CallOrAccess, vec![nt(PrimaryExpr)]));
    // Llamada a función/functor:  expr(args)
    g.add(p(CallOrAccess, vec![
        nt(CallOrAccess), t(LParen), nt(ArgList), t(RParen),
    ]));
    // Acceso a atributo:  expr.id
    g.add(p(CallOrAccess, vec![
        nt(CallOrAccess), t(Dot), t(Identifier),
    ]));
    // Llamada a método:  expr.id(args)
    g.add(p(CallOrAccess, vec![
        nt(CallOrAccess), t(Dot), t(Identifier),
        t(LParen), nt(ArgList), t(RParen),
    ]));
    // Indexación:  expr[expr]
    g.add(p(CallOrAccess, vec![
        nt(CallOrAccess), t(LBracket), nt(Expr), t(RBracket),
    ]));

    // ── 10.14 PrimaryExpr ───────────────────────────────────────────────
    // Literales
    g.add(p(PrimaryExpr, vec![t(Number)]));
    g.add(p(PrimaryExpr, vec![t(String)]));
    g.add(p(PrimaryExpr, vec![t(Char)]));
    g.add(p(PrimaryExpr, vec![t(True)]));
    g.add(p(PrimaryExpr, vec![t(False)]));
    g.add(p(PrimaryExpr, vec![t(Null)]));
    // Identificador (variable o función global)
    g.add(p(PrimaryExpr, vec![t(Identifier)]));
    // `base` — referencia al método del padre dentro de un método
    g.add(p(PrimaryExpr, vec![t(Base)]));
    // Subexpresión parentizada
    g.add(p(PrimaryExpr, vec![t(LParen), nt(Expr), t(RParen)]));
    // Expresiones compuestas como átomos
    g.add(p(PrimaryExpr, vec![nt(BlockExpr)]));
    g.add(p(PrimaryExpr, vec![nt(LetExpr)]));
    g.add(p(PrimaryExpr, vec![nt(IfExpr)]));
    g.add(p(PrimaryExpr, vec![nt(WhileExpr)]));
    g.add(p(PrimaryExpr, vec![nt(ForExpr)]));
    g.add(p(PrimaryExpr, vec![nt(NewExpr)]));
    // Vector literal
    g.add(p(PrimaryExpr, vec![nt(VectorLiteral)]));

    // ═══════════════════════════════════════════════════════════════════
    //  11. Expresiones compuestas
    // ═══════════════════════════════════════════════════════════════════

    // ── BlockExpr ───────────────────────────────────────────────────────
    // { ExprList }        — sin `;` final
    // { ExprList ; }      — con `;` final (igualmente válido en HULK)
    g.add(p(BlockExpr, vec![t(LBrace), nt(ExprList), t(RBrace)]));
    g.add(p(BlockExpr, vec![t(LBrace), nt(ExprList), t(Semicolon), t(RBrace)]));

    // ExprList — lista de expresiones separadas por `;`
    // El valor del bloque es la última expresión.
    g.add(p(ExprList, vec![nt(Expr)]));
    g.add(p(ExprList, vec![nt(ExprList), t(Semicolon), nt(Expr)]));

    // ── LetExpr ─────────────────────────────────────────────────────────
    // let LetBindingList in Expr
    g.add(p(LetExpr, vec![t(Let), nt(LetBindingList), t(In), nt(Expr)]));

    // LetBindingList → LetBinding  |  LetBindingList , LetBinding
    g.add(p(LetBindingList, vec![nt(LetBinding)]));
    g.add(p(LetBindingList, vec![nt(LetBindingList), t(Comma), nt(LetBinding)]));

    // LetBinding → id = Expr  |  id : TypeName = Expr
    g.add(p(LetBinding, vec![
        t(Identifier), t(Assign), nt(Expr),
    ]));
    g.add(p(LetBinding, vec![
        t(Identifier), t(Colon), nt(TypeName), t(Assign), nt(Expr),
    ]));

    // ── IfExpr ──────────────────────────────────────────────────────────
    // En HULK el `else` es OBLIGATORIO — no hay dangling-else.
    //
    // if ( Expr ) Expr ElifChain else Expr
    //
    // ElifChain → ε
    //           | ElifChain elif ( Expr ) Expr
    g.add(p(IfExpr, vec![
        t(If), t(LParen), nt(Expr), t(RParen),
        nt(Expr),
        nt(ElifChain),
        t(Else), nt(Expr),
    ]));

    g.add(eps(ElifChain));
    g.add(p(ElifChain, vec![
        nt(ElifChain),
        t(Elif), t(LParen), nt(Expr), t(RParen),
        nt(Expr),
    ]));

    // ── WhileExpr ───────────────────────────────────────────────────────
    // while ( Expr ) Expr
    g.add(p(WhileExpr, vec![
        t(While), t(LParen), nt(Expr), t(RParen), nt(Expr),
    ]));

    // ── ForExpr ─────────────────────────────────────────────────────────
    // for ( id in Expr ) Expr
    g.add(p(ForExpr, vec![
        t(For), t(LParen), t(Identifier), t(In), nt(Expr), t(RParen),
        nt(Expr),
    ]));

    // ── NewExpr ─────────────────────────────────────────────────────────
    // new TypeName ( ArgList )
    // NOTA: TypeName aquí solo admite la forma simple `IDENTIFIER`;
    // la restricción es semántica.
    g.add(p(NewExpr, vec![
        t(New), nt(TypeName), t(LParen), nt(ArgList), t(RParen),
    ]));

    // ═══════════════════════════════════════════════════════════════════
    //  12. Vectores
    //
    //  Forma explícita:   [ e1, e2, e3 ]
    //  Forma vacía:       [ ]
    //  Forma generadora:  [ expr | id in expr ]
    //
    //  ⚠ CONFLICTO CONOCIDO:
    //  La forma generadora usa `|` como separador, el mismo token que el
    //  operador lógico OR en OrExpr.  Dentro de `[Expr | ...]` el parser
    //  LALR(1) tiene un shift/reduce conflict en el token `|`:
    //    – shift `|` como separador del generador
    //    – shift `|` para continuar construyendo OrExpr → OrExpr | AndExpr
    //  Se resuelve en `table_builder.rs` con una regla de desambiguación:
    //  cuando el stack tiene `[ Expr` y el lookahead es `|` seguido de
    //  `IDENTIFIER in`, se elige la producción generadora.
    //  Si el lookahead es `|` sin ese patrón, se sigue construyendo OrExpr.
    // ═══════════════════════════════════════════════════════════════════

    // VectorLiteral → [ ]
    g.add(p(VectorLiteral, vec![t(LBracket), t(RBracket)]));

    // VectorLiteral → [ ArgListNonEmpty ]
    g.add(p(VectorLiteral, vec![
        t(LBracket), nt(ArgListNonEmpty), t(RBracket),
    ]));

    // VectorLiteral → [ Expr | id in Expr ]   (generador)
    g.add(p(VectorLiteral, vec![
        t(LBracket),
        nt(Expr), t(Or), t(Identifier), t(In), nt(Expr),
        t(RBracket),
    ]));

    g
}

// ─────────────────────────────────────────────
//  Tests de cordura sobre la gramática
// ─────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use super::super::symbol::NonTerminal;

    fn grammar() -> Grammar { build() }

    #[test]
    fn grammar_has_productions() {
        let g = grammar();
        assert!(g.len() > 50, "La gramática debe tener más de 50 producciones");
    }

    #[test]
    fn start_production_is_first() {
        let g = grammar();
        let p = g.production(0);
        assert_eq!(p.head, NonTerminal::Start);
        assert_eq!(p.body.len(), 2); // Program Eof
    }

    #[test]
    fn every_nonterminal_has_at_least_one_production() {
        use NonTerminal::*;
        let g = grammar();
        let required = [
            Program, DeclList, Decl, FuncDecl, TypeDecl, ProtocolDecl,
            ParamList, ParamListNonEmpty, Param, ArgList, ArgListNonEmpty,
            TypeName, TypeArgs, TypeArgList, InheritClause,
            TypeBody, TypeMemberList, TypeMember, AttributeDef, MethodDef,
            ProtocolBody, ProtocolMemberList, ProtocolMember, MethodSignature,
            Expr, AssignExpr, OrExpr, AndExpr, CompareExpr, IsAsExpr,
            ConcatExpr, AddExpr, MulExpr, PowerExpr, UnaryExpr,
            PostfixExpr, CallOrAccess, PrimaryExpr,
            BlockExpr, ExprList, LetExpr, LetBindingList, LetBinding,
            IfExpr, ElifChain, WhileExpr, ForExpr, NewExpr,
            VectorLiteral,
        ];
        for nt in required {
            assert!(
                !g.productions_for(&nt).is_empty(),
                "No hay producciones para {:?}", nt
            );
        }
    }

    #[test]
    fn epsilon_productions_exist_where_expected() {
        use NonTerminal::*;
        let g = grammar();
        let should_have_epsilon = [DeclList, ParamList, ArgList, TypeArgs,
                                   InheritClause, TypeMemberList,
                                   ProtocolMemberList, ElifChain];
        for nt in should_have_epsilon {
            let has_eps = g.productions_for(&nt)
                .iter()
                .any(|&id| g.production(id).is_epsilon());
            assert!(has_eps, "{:?} debería tener producción ε", nt);
        }
    }

    #[test]
    fn print_grammar_summary() {
        let g = grammar();
        println!("Total de producciones: {}", g.len());
        g.dump();
    }
}