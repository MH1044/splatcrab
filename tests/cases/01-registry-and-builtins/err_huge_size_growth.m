% covers: 14 - indexed growth is guarded by the same check as the constructors
% NOTE: this is the one non-builtin path the panic guard reaches: growing on
% NOTE: assignment goes through args::check_size in interp.rs. The ':' operator
% NOTE: still does not, which is the open row in ARCHITECTURE.md's Known bugs.
a = 1;
disp(a)
a(1e10) = 5;
