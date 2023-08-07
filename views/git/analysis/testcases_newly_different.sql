-- this view shows every testcase that has not failed on the parent commit, but has failed on the actual commit
create materialized view testcases_newly_different
AS
SELECT overview.commit,

       -- WARNING: in case there are multiple tests with same suite name and testcase name, those may multiply in this view

       coalesce(STRING_AGG(retest_all.name,
                           E'\n'
                           ORDER BY retest_all.name), '') as retest_all_different,

       coalesce(STRING_AGG(dynamic.name,
                           E'\n'
                           ORDER BY dynamic.name), '')           as dynamic_different,

       coalesce(STRING_AGG(static.name,
                           E'\n'
                           ORDER BY static.name), '')            as static_different


FROM (((
    (testcase_overview overview join testcase_parent_overview parent
     on overview.commit = parent.commit
         and overview.retest_all_name = parent.retest_all_name
         and overview.retest_all_suite_name = parent.retest_all_suite_name)
        left outer join "TestCase" retest_all
    on overview.retest_all_testcase_id = retest_all.id
        AND not exists(SELECT *
                       FROM "TestCase" retest_all_parent
                       WHERE parent.retest_all_testcase_id = retest_all_parent.id
                         AND retest_all_parent.status = retest_all.status))
    left outer join "TestCase" dynamic
       on overview.dynamic_testcase_id = dynamic.id
           AND not exists(SELECT *
                          FROM "TestCase" dynamic_parent
                          WHERE parent.dynamic_testcase_id = dynamic_parent.id
                            AND dynamic_parent.status = dynamic.status))
    left outer join
    "TestCase" static on overview.static_testcase_id = static.id
    AND not exists(SELECT *
                   FROM "TestCase" static_parent
                   WHERE parent.static_testcase_id = static_parent.id
                     AND static_parent.status = static.status))
GROUP BY overview.commit, overview.retest_all_id, overview.dynamic_id,
         overview.static_id;