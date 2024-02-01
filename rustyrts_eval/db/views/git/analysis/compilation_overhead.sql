--CREATE VIEW compilation_overhead as
(SELECT r.path,
        r.id                                                  as repo_id,
        ROUND(CAST(avg((t.dynamic_build_duration - t.retest_all_build_duration) * 100.0 /
                       t.retest_all_build_duration) as numeric), 2) as overhead_dynamic,
        ROUND(CAST(avg((t.static_build_duration - t.retest_all_build_duration) * 100.0 /
                       t.retest_all_build_duration) as numeric), 2) as overhead_static
 FROM testreport_extended t
          join "Commit" c on t.commit = c.id
          join "Repository" r on r.id = c.repo_id
 group by r.path, r.id)
UNION
(SELECT 'all'                                                 as path,
        null                                                  as repo_id,
        ROUND(CAST(avg((t.dynamic_build_duration - t.retest_all_build_duration) * 100.0 /
                       t.retest_all_build_duration) as numeric), 2) as overhead_dynamic,
        ROUND(CAST(avg((t.static_build_duration - t.retest_all_build_duration) * 100.0 /
                       t.retest_all_build_duration) as numeric), 2) as overhead_static
 FROM testreport_extended t)
--;
