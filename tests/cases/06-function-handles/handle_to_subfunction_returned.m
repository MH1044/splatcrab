% covers: 13 - a handle made inside a path file to that file's subfunction keeps calling it from this script, where the name does not resolve
% makeh.m, beside this script, returns @hidden, and hidden is a subfunction
% private to makeh.m (as 05's err_subfunction_private shows for a call by
% name), so only the binding made when the handle was created reaches it.
h = makeh();
disp(h(4))
