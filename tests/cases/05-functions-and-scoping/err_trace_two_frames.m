% covers: 6 - an uncaught error's trace has one stack line per function frame, innermost first, each naming the line its frame was running
% Lines count this covers line as line 1: inner_fn fails on line 10, and
% outer_fn was running its call to inner_fn on line 7, and the Error:
% prefix names the script's own statement, line 5.
outer_fn(1)
function outer_fn(x)
inner_fn(x)
end
function inner_fn(x)
y = x + undefined_q;
end
