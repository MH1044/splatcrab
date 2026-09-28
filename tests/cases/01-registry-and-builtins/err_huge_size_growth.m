% covers: 14 - indexed growth is guarded by the same check as the constructors
% NOTE: growing on assignment is guarded by the same check as the constructors:
% NOTE: since cycle 03 the grown size goes through args::check_shape in interp.rs.
a = 1;
disp(a)
a(1e10) = 5;
