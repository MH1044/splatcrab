% covers: Scope - eval goes through the shared nesting budget, so eval recursing through a function stops at the recursion limit
f(1)
function f(n)
  eval('f(n + 1)');
end
