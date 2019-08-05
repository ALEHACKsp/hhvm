<?hh // partial

// This doc comment block generated by idl/sysdoc.php
/**
 * ( excerpt from http://php.net/manual/en/class.splobserver.php )
 *
 * The SplObserver interface is used alongside SplSubject to implement the
 * Observer Design Pattern.
 *
 */
interface SplObserver {
  // This doc comment block generated by idl/sysdoc.php
  /**
   * ( excerpt from http://php.net/manual/en/splobserver.update.php )
   *
   * This method is called when any SplSubject to which the observer is
   * attached calls SplSubject::notify(). Warning: This function is currently
   * not documented; only its argument list is available.
   *
   * @subject    mixed   The SplSubject notifying the observer of an update.
   *
   * @return     mixed   No value is returned.
   */
  public function update ( SplSubject $subject );
}
