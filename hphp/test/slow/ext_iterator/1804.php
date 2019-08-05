<?hh

function getFiles(&$rdi,$depth=0) {
  if (!is_object($rdi)) return;
  $files = array();
  // order changes per machine
  for ($rdi->rewind(); $rdi->valid(); $rdi->next()) {
    if ($rdi->isDot()) continue;
    if ($rdi->isDir() || $rdi->isFile()) {
      $indent = '';
      for ($i = 0; $i<=$depth; ++$i) $indent .= " ";
      $files[] = $indent.$rdi->current()."\n";
      if ($rdi->hasChildren()) {
        $children = $rdi->getChildren();
        getFiles(&$children, 1+$depth);
      }
    }
  }
  asort(&$files);
  var_dump(array_values($files));
}

<<__EntryPoint>>
function main_1804() {
  $rdi = new RecursiveDirectoryIterator(__DIR__.'/../../sample_dir');
  getFiles(&$rdi);
}
