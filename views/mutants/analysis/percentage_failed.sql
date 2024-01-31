CREATE VIEW percentage_failed as
(SELECT commit,
        avg(retest_all_count_failed * 100 / retest_all_count) as retest_all,
        avg(dynamic_count_failed * 100.0 / dynamic_count)     as dynamic,
        avg(static_count_failed * 100 / static_count)         as static
 FROM testcases_count
 WHERE retest_all_count != 0
   and dynamic_count != 0
   and static_count != 0
 group by commit)
UNION
(SELECT 0, -- averaged over all mutants
        avg(retest_all_count_failed * 100 / retest_all_count),
        avg(dynamic_count_failed * 100.0 / dynamic_count),
        avg(static_count_failed * 100 / static_count)
 FROM testcases_count
 WHERE retest_all_count != 0
   and dynamic_count != 0
   and static_count != 0);