create materialized view testcases_selected
AS
SELECT overview.commit,
       overview.retest_all_mutant_id,
       overview.descr                                            as descr,

       coalesce(STRING_AGG(retest_all_failed.name,
                           E'\n'
                           ORDER BY retest_all_failed.name), '') as retest_all,

       coalesce(STRING_AGG(dynamic_failed.name,
                           E'\n'
                           ORDER BY dynamic_failed.name), '')    as dynamic,

       coalesce(STRING_AGG(static_failed.name,
                           E'\n'
                           ORDER BY static_failed.name), '')     as static


FROM (((mutant_testcase_overview overview left outer join "MutantsTestCase" retest_all_failed
        on overview.retest_all_testcase_id = retest_all_failed.id)
    left outer join "MutantsTestCase" dynamic_failed
       on overview.dynamic_testcase_id = dynamic_failed.id)
    left outer join
    "MutantsTestCase" static_failed on overview.static_testcase_id = static_failed.id)
GROUP BY overview.commit, overview.descr, overview.retest_all_mutant_id, overview.dynamic_mutant_id,
         overview.static_mutant_id;