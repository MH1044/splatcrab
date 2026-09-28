% covers: Scope, Value::Func - a long chain of handles, each capturing the one before, is freed
% without recursion: 500,000 links aborted with a stack overflow before cycle 06's review fix
% (exit 134 on Linux). The g = h form, and the drop at exit, are pinned by a unit test
h = @() 1; for k = 1:500000, h = @() h() + 1; end; clear h
disp(1)
