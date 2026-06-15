import { Component, inject } from '@angular/core';
import { MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatButtonModule } from '@angular/material/button';

@Component({
  selector: 'app-privacy-consent-dialog',
  standalone: true,
  imports: [MatDialogModule, MatButtonModule],
  templateUrl: './privacy-consent-dialog.html',
})
export class PrivacyConsentDialog {
  private dialogRef = inject(MatDialogRef<PrivacyConsentDialog>);

  accept(): void {
    this.dialogRef.close(true);
  }

  decline(): void {
    this.dialogRef.close(false);
  }
}