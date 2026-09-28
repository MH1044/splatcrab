% covers: 9 - an anonymous function can return another, which keeps the value it captured: add(3) remembers a = 3
add = @(a) @(b) a + b; add3 = add(3); disp(add3(4))
