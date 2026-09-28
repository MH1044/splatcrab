% covers: 7 - class names a handle's class function_handle, and isa(f, 'function_handle') is a logical 1, which disp prints four wide
f = @(x) x + 1; disp(class(f)); disp(isa(f, 'function_handle'))
