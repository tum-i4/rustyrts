-- this view shows every testcase that has been executed on the actual commit
--create materialized view testcases_selected AS
SELECT overview.commit,

       -- WARNING: in case there are multiple tests with same suite name and testcase name, those may multiply in this view

       coalesce(STRING_AGG(retest_all_failed.name,
                           E'\n'
                           ORDER BY retest_all_failed.name), '') as retest_all,

       coalesce(STRING_AGG(dynamic_failed.name,
                           E'\n'
                           ORDER BY dynamic_failed.name), '')    as dynamic,

       coalesce(STRING_AGG(static_failed.name,
                           E'\n'
                           ORDER BY static_failed.name), '')     as static


FROM (((testcase_overview overview left outer join "TestCase" retest_all_failed
        on overview.retest_all_testcase_id = retest_all_failed.id)
    left outer join "TestCase" dynamic_failed
       on overview.dynamic_testcase_id = dynamic_failed.id)
    left outer join
    "TestCase" static_failed on overview.static_testcase_id = static_failed.id)
GROUP BY overview.commit, overview.retest_all_id, overview.dynamic_id,
         overview.static_id
--;