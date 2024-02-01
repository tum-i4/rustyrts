-- this view shows every testcase that has failed on the mutant
--create materialized view testcases_failed AS
SELECT overview.commit,
       overview.retest_all_mutant_id,
       overview.descr                                            as descr,

       -- WARNING: in case there are multiple tests with same suite name and testcase name, those may multiply in this view

       coalesce(STRING_AGG(retest_all_failed.name,
                           E'\n'
                           ORDER BY retest_all_failed.name), '') as retest_all_failed,

       coalesce(STRING_AGG(dynamic_failed.name,
                           E'\n'
                           ORDER BY dynamic_failed.name), '')    as dynamic_failed,

       coalesce(STRING_AGG(static_failed.name,
                           E'\n'
                           ORDER BY static_failed.name), '')     as static_failed


FROM (((mutant_testcase_overview overview left outer join "MutantsTestCase" retest_all_failed
        on overview.retest_all_testcase_id = retest_all_failed.id
            AND retest_all_failed.status =
                'FAILED')
    left outer join "MutantsTestCase" dynamic_failed
       on overview.dynamic_testcase_id = dynamic_failed.id
           AND dynamic_failed.status = 'FAILED')
    left outer join
    "MutantsTestCase" static_failed on overview.static_testcase_id = static_failed.id
    AND static_failed.status = 'FAILED')
GROUP BY overview.commit, overview.descr, overview.retest_all_mutant_id, overview.dynamic_mutant_id,
         overview.static_mutant_id
--;