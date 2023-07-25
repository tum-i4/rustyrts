create materialized view testcases_newly_failed
AS
SELECT overview.commit,

       coalesce(STRING_AGG(retest_all_failed.name,
                           E'\n'
                           ORDER BY retest_all_failed.name), '') as retest_all_failed,

       coalesce(STRING_AGG(dynamic_failed.name,
                           E'\n'
                           ORDER BY dynamic_failed.name), '')    as dynamic_failed,

       coalesce(STRING_AGG(static_failed.name,
                           E'\n'
                           ORDER BY static_failed.name), '')     as static_failed


FROM (((
    (testcase_overview overview join testcase_parent_overview parent
     on overview.commit = parent.commit
         and overview.retest_all_name = parent.retest_all_name
         and overview.retest_all_suite_name = parent.retest_all_suite_name)
        left outer join "TestCase" retest_all_failed
    on overview.retest_all_testcase_id = retest_all_failed.id
        AND retest_all_failed.status =
            'FAILED'
        AND not exists(SELECT *
                       FROM "TestCase" retest_all_parent
                       WHERE parent.retest_all_testcase_id = retest_all_parent.id
                         AND retest_all_parent.status = 'FAILED'))
    left outer join "TestCase" dynamic_failed
       on overview.dynamic_testcase_id = dynamic_failed.id
           AND dynamic_failed.status = 'FAILED'
           AND not exists(SELECT *
                          FROM "TestCase" dynamic_parent
                          WHERE parent.dynamic_testcase_id = dynamic_parent.id
                            AND dynamic_parent.status = 'FAILED'))
    left outer join
    "TestCase" static_failed on overview.static_testcase_id = static_failed.id
    AND static_failed.status = 'FAILED'
    AND not exists(SELECT *
                   FROM "TestCase" static_parent
                   WHERE parent.static_testcase_id = static_parent.id
                     AND static_parent.status = 'FAILED'))
GROUP BY overview.commit, overview.retest_all_id, overview.dynamic_id,
         overview.static_id;