% covers: 4 - catch e binds the error: e.message is its text, and error with one argument leaves e.identifier empty
% isempty returns a logical, so disp shows it four wide (cycle 02).
try, error('boom'), catch e, disp(e.message), disp(isempty(e.identifier)), end
