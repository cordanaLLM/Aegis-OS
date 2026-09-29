// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
//
// HISS-01 for the JavaScript and Svelte under ui/ (D100): a function must not
// call itself.
//
// Reported: a call whose callee resolves, through ESLint's scope analysis, to
// a function the call sits inside -- a function declaration, a named function
// expression, or a function or arrow assigned to a const/let/var -- whether
// the call is in that function's own body or in a closure nested inside it;
// and `this.name()` inside a method called `name`.
//
// Not reported, and stated here so nobody reads more into a clean lint:
// mutual recursion (f calls g, g calls f), recursion through a value passed
// as an argument, through `arguments.callee`, through a computed member or
// through an object other than `this`. There is no call-graph check; that is
// the scanner cordanaLLM/praetor#589 asks for.

const FUNCTIONS = new Set(['FunctionDeclaration', 'FunctionExpression', 'ArrowFunctionExpression']);
const METHODS = new Set(['MethodDefinition', 'Property']);
// Scalar bound on the ancestors inspected for one call (HISS-02).
const MAX_DEPTH = 256;

// The function node a resolved variable names, or null when it names none.
function namedFunction(variable) {
  const definition = variable === null || variable === undefined ? undefined : variable.defs[0];
  if (definition === undefined) {
    return null;
  }
  if (definition.type === 'FunctionName') {
    return definition.node;
  }
  const init = definition.type === 'Variable' ? definition.node.init : null;
  return init !== null && FUNCTIONS.has(init.type) ? init : null;
}

// The variable an identifier callee refers to, from the scope the call is in.
function calleeVariable(sourceCode, node) {
  const scope = sourceCode.getScope(node);
  const reference = scope.references.find((entry) => entry.identifier === node.callee);
  return reference === undefined ? null : reference.resolved;
}

// The name of the method whose body the call is directly in, or null.
function enclosingMethodName(ancestors) {
  const bounded = ancestors.slice(-MAX_DEPTH);
  for (let index = bounded.length - 1; index > 0; index -= 1) {
    const node = bounded[index];
    if (node.type === 'FunctionExpression' || node.type === 'FunctionDeclaration') {
      const owner = bounded[index - 1];
      const isMethod = METHODS.has(owner.type) && owner.value === node && !owner.computed;
      return isMethod && owner.key.type === 'Identifier' ? owner.key.name : null;
    }
  }
  return null;
}

function isThisCall(callee) {
  return (
    callee.type === 'MemberExpression' &&
    callee.object.type === 'ThisExpression' &&
    !callee.computed &&
    callee.property.type === 'Identifier'
  );
}

export default {
  meta: {
    type: 'problem',
    docs: { description: 'HISS-01: a function must not call itself by name (D100)' },
    schema: [],
    messages: {
      selfCall: "HISS-01: '{{name}}' calls itself; recursion is prohibited, use a bounded loop",
    },
  },
  create(context) {
    const sourceCode = context.sourceCode;
    return {
      CallExpression(node) {
        const ancestors = sourceCode.getAncestors(node).slice(-MAX_DEPTH);
        if (node.callee.type === 'Identifier') {
          const target = namedFunction(calleeVariable(sourceCode, node));
          if (target !== null && ancestors.includes(target)) {
            context.report({ node, messageId: 'selfCall', data: { name: node.callee.name } });
          }
          return;
        }
        if (isThisCall(node.callee) && enclosingMethodName(ancestors) === node.callee.property.name) {
          context.report({ node, messageId: 'selfCall', data: { name: node.callee.property.name } });
        }
      },
    };
  },
};
